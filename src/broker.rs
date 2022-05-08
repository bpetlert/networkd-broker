use crate::{
    dbus_interface::NetworkManagerProxy,
    environment::Environments,
    launcher::Launcher,
    link::{LinkDetails, LinkEvent},
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
        debug!("Start script launcher");
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

    pub async fn trigger_all(&self) -> Result<()> {
        let conn = Connection::system().await?;
        let proxy = NetworkManagerProxy::new(&conn).await?;
        let links = proxy.list_links().await?;
        for (index, name, path) in links {
            info!("run-startup-triggers on '{name}'");

            let describe_link = proxy.describe_link(index).await?;

            let link_details = match serde_json::from_str::<LinkDetails>(&describe_link) {
                Ok(link_details) => link_details,
                Err(err) => return Err(anyhow!("Cannot get link state of {name}: {err}")),
            };

            let event = Box::new(LinkEvent {
                iface: name,
                state: link_details.operational_state.clone(),
                path: path.to_string(),
                link_details,
                link_details_json: describe_link,
            });

            if let Err(err) = self.respond(&event) {
                warn!("{err}");
            }
        }

        info!("Finished 'run-startup-triggers'");
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
