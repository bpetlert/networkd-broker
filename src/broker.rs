use crate::{
    dbus_interface::NetworkManagerProxy,
    launcher::Launcher,
    link::{LinkDetails, LinkEvent},
    script::{EnvVar, ScriptBuilder},
};
use anyhow::{bail, Context, Result};
use futures_util::stream::StreamExt;
use libsystemd::daemon::{self, NotifyState};
use std::{collections::BTreeMap, path::PathBuf, sync::Arc};
use tracing::{debug, error, info, warn};
use zbus::{Connection, Message, MessageStream};

/// A responder manages link event
#[derive(Debug)]
pub struct Broker {
    script_root_dir: PathBuf,
    script_timeout: u64,
    launcher: Launcher,
    dbus_conn: Connection,
    link_state_cache: BTreeMap<String, String>,
}

impl Broker {
    pub async fn new(script_root_dir: PathBuf, script_timeout: u64) -> Result<Broker> {
        debug!("Start script launcher");
        let launcher = Launcher::new();

        debug!("Connect to System DBus");
        let dbus_conn = Connection::system().await?;

        debug!("Initialize link state cache");
        let link_state_cache = Broker::init_link_state_cache(&dbus_conn).await?;

        Ok(Broker {
            script_root_dir,
            script_timeout,
            launcher,
            dbus_conn,
            link_state_cache,
        })
    }

    pub async fn listen(&mut self) -> Result<()> {
        debug!("Create filter proxy");
        let proxy = zbus::fdo::DBusProxy::new(&self.dbus_conn).await?;
        if let Err(err) = proxy
            .add_match(
                "\
                type='signal',\
                interface='org.freedesktop.DBus.Properties',\
                member='PropertiesChanged',\
                path_namespace='/org/freedesktop/network1/link'",
            )
            .await
        {
            bail!("Cannot crate filter proxy, {err}");
        }

        debug!("Create message stream");
        let mut stream = MessageStream::from(&self.dbus_conn);

        debug!("Notify systemd that we are ready :)");
        let _ = daemon::notify(false, &[NotifyState::Ready])
            .context("Failed to notify systemd: ready")?;

        debug!("Start listening to link events...");
        let _ = daemon::notify(
            false,
            &[NotifyState::Status(
                "Start listening to link events...".to_string(),
            )],
        )
        .context("Failed to notify systemd: Start listening to link events...")?;

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
                Err(err) => bail!("Cannot get link state of {name}: {err}"),
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
        let script_path = self.script_root_dir.join(state_dir);
        let scripts = match ScriptBuilder::build_from(script_path, None, None) {
            Ok(s) => s,
            Err(err) => bail!("{err}"),
        };

        // Push scripts with args + envs to launcher's queue.
        for script in scripts {
            let script = script
                .set_arg0(&event.state.clone())
                .set_arg1(&event.iface.clone())
                .add_env(EnvVar::DeviceIface(event.iface.clone()))
                .add_env(EnvVar::BrokerAction(event.state.clone()))
                .add_env(EnvVar::Json(event.link_details_json.clone()))
                .set_default_timeout(self.script_timeout)
                .build();
            if let Err(err) = self.launcher.add(script) {
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
                Err(err) => bail!("Cannot get link state of {name}: {err}"),
            };

            cache.insert(name, link_details.operational_state);
        }
        Ok(cache)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_std::task;
    use duct::cmd;

    #[test]
    fn test_init_link_state_cache() {
        // Get all network links using NetworkctlCtl command
        let stdout = cmd!("networkctl", "--no-pager", "--no-legend", "list")
            .pipe(cmd!("awk", "{ print $2, $4 }"))
            .read()
            .unwrap();
        let links: Vec<Vec<&str>> = stdout
            .lines()
            .into_iter()
            .map(|line| line.split(' ').collect())
            .collect();

        task::block_on(async {
            let dbus_conn = Connection::system().await.unwrap();
            let cache = Broker::init_link_state_cache(&dbus_conn).await.unwrap();
            for link in links {
                assert_eq!(cache.get(link[0]), Some(&link[1].to_string()));
            }
        });
    }
}
