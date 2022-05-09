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
use std::{collections::BTreeMap, path::PathBuf, sync::Arc};
use tracing::{debug, error, info, warn};
use zbus::{Connection, Message, MessageStream};

/// A responder manages link event
#[derive(Debug)]
pub struct Broker {
    script_dir: PathBuf,
    timeout: u64,
    launcher: Launcher,
    dbus_conn: Connection,
    link_state_cache: BTreeMap<String, String>,
}

impl Broker {
    pub async fn new<P>(script_dir: P, timeout: u64) -> Result<Broker>
    where
        P: Into<PathBuf>,
    {
        debug!("Start script launcher");
        let launcher = Launcher::new();

        debug!("Connect to System DBus");
        let dbus_conn = Connection::system().await?;

        debug!("Initialize link state cache");
        let link_state_cache = Broker::init_link_state_cache(&dbus_conn).await?;

        Ok(Broker {
            script_dir: script_dir.into(),
            timeout,
            launcher,
            dbus_conn,
            link_state_cache,
        })
    }

    pub async fn listen(&mut self) -> Result<()> {
        debug!("Create filter proxy");
        let proxy = zbus::fdo::DBusProxy::new(&self.dbus_conn).await?;
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
        let mut stream = MessageStream::from(&self.dbus_conn);

        debug!("Notify systemd that we are ready :)");
        let _ = daemon::notify(false, &[NotifyState::Ready]).expect("Notify systemd: ready");

        debug!("Start listening for link event...");
        let _ = daemon::notify(
            false,
            &[NotifyState::Status(
                "Start listening for link event...".to_string(),
            )],
        )
        .expect("Notify systemd: Start listening for link event...");

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

                match LinkEvent::new(&msg, &self.dbus_conn).await {
                    Ok(link_event) => {
                        debug!("Link Event: {link_event}");

                        match self.link_state_cache.get_mut(&link_event.iface) {
                            Some(previous_operational_state) => {
                                if *previous_operational_state == link_event.state {
                                    debug!("Skip event, no change in OperationalState");
                                    continue;
                                }

                                debug!("Update link state cache of {}", link_event.iface);
                                *previous_operational_state = link_event.state.clone();
                            }
                            None => {
                                debug!("Insert new link state cache");
                                self.link_state_cache
                                    .insert(link_event.iface.clone(), link_event.state.clone());
                            }
                        }

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
        let proxy = NetworkManagerProxy::new(&self.dbus_conn).await?;
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

    async fn init_link_state_cache(conn: &Connection) -> Result<BTreeMap<String, String>> {
        let proxy = NetworkManagerProxy::new(conn).await?;
        let links = proxy.list_links().await?;
        let mut cache: BTreeMap<String, String> = BTreeMap::new();
        for (index, name, _path) in links {
            let describe_link = proxy.describe_link(index).await?;

            let link_details = match serde_json::from_str::<LinkDetails>(&describe_link) {
                Ok(link_details) => link_details,
                Err(err) => return Err(anyhow!("Cannot get link state of {name}: {err}")),
            };

            cache.insert(name, link_details.operational_state);
        }
        Ok(cache)
    }
}
