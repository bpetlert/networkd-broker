use crate::{
    environment::Environments,
    launcher::Launcher,
    link::LinkEvent,
    script::{Script, ScriptArguments},
};

use anyhow::{anyhow, Result};
use futures_util::stream::StreamExt;
use libsystemd::daemon::{self, NotifyState};
use std::{path::PathBuf, sync::Arc};
use tracing::{debug, error, info, warn};
use zbus::{Connection, Message, MessageStream};

/// A responder manages link event
#[derive(Debug)]
pub struct Broker {
    script_dir: PathBuf,
    timeout: u64,
    launcher: Launcher,
}

impl Broker {
    pub fn new<P>(script_dir: P, timeout: u64) -> Broker
    where
        P: Into<PathBuf>,
    {
        // Start script launcher
        let launcher = Launcher::new();

        Broker {
            script_dir: script_dir.into(),
            timeout,
            launcher,
        }
    }

    pub async fn listen(&self) -> Result<()> {
        debug!("Connect to System DBus");
        let conn = Connection::system().await?;

        debug!("Create filter proxy");
        let proxy = zbus::fdo::DBusProxy::new(&conn).await?;
        proxy
            .add_match(
                "\
                type='signal',\
                interface='org.freedesktop.DBus.Properties',\
                member='PropertiesChanged',\
                path_namespace='/org/freedesktop/network1/link'",
            )
            .await?;

        debug!("Create message stream");
        let mut stream = MessageStream::from(&conn);

        debug!("Notify systemd that we are ready :)");
        let _ = daemon::notify(false, &[NotifyState::Ready]);

        info!("Start listening for link event.");
        futures_util::try_join!(async {
            while let Some(msg) = stream.next().await {
                let msg: Arc<Message> = match msg {
                    Ok(m) => {
                        debug!("New message: {m}");
                        m
                    }
                    Err(err) => {
                        error!("{err}");
                        continue;
                    }
                };

                match LinkEvent::new(&msg, &conn).await {
                    Ok(link_event) => {
                        debug!("Link Event: {link_event}");
                        if let Err(err) = self.respond(&link_event) {
                            warn!("{err}");
                        }
                    }
                    Err(err) => debug!("{err}"),
                }
            }
            Ok::<(), zbus::Error>(())
        },)?;

        Ok(())
    }

    // TODO: Rewrite this
    pub fn trigger_all(&self) -> Result<()> {
        // let link_list = match Link::link_list() {
        //     Ok(link) => link,
        //     Err(err) => {
        //         return Err(anyhow!(
        //             "Cannot trigger all interface, since no iface found, {}",
        //             err
        //         ));
        //     }
        // };

        // info!("Start trigger all interfaces.");
        // for (idx, link) in link_list.iter() {
        //     info!("Trigger on interface `{}`", link.iface);
        //     if let Ok(path) = dbus::Path::new(LinkEvent::index_to_dbus_path(*idx)) {
        //         // Create fake event
        //         let mut event = LinkEvent {
        //             path,
        //             state_type: StateType::OperationalState,
        //             state: link.operational.clone(),
        //         };

        //         // 1: OperationalState
        //         if let Err(e) = self.respond(&event) {
        //             warn!("{}", e);
        //         }

        //         // 2: AdministrativeState
        //         event.state_type = StateType::AdministrativeState;
        //         event.state = link.setup.clone();
        //         if let Err(e) = self.respond(&event) {
        //             warn!("{}", e);
        //         }
        //     } else {
        //         // Display error and skip this interface.
        //         warn!("Cannot create D-Bus path from link index `{}`.", idx);
        //     }
        // }

        Ok(())
    }

    fn respond(&self, event: &LinkEvent) -> Result<()> {
        info!("Respond to '{}' event of '{}'", &event.state, &event.iface);

        // Get all scripts associated with current event
        let state_dir = format!("{}.d", event.state);
        let script_path = self.script_dir.join(state_dir);
        let scripts = match Script::get_scripts_in(&script_path, None, None) {
            Ok(s) => s,
            Err(err) => {
                return Err(anyhow!("{err}"));
            }
        };

        // Build script's arguments
        let mut args = ScriptArguments::new();
        args.state = event.state.clone();
        args.iface = event.iface.clone();
        let shared_args = Arc::new(args);

        // Pack all event-related environments.
        let mut envs = Environments::new();
        envs.pack_from(event)?;
        let shared_envs = Arc::new(envs);

        // Push scripts with args + envs to launcher's queue.
        for mut s in scripts {
            s.args = Some(shared_args.clone());
            s.envs = Some(shared_envs.clone());
            s.timeout = self.timeout;
            if let Err(err) = self.launcher.add(s) {
                warn!("{err}");
            }
        }

        Ok(())
    }
}
