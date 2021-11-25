use crate::{
    environment::Environments,
    launcher::Launcher,
    link::{Link, LinkEvent, StateType},
    script::{Arguments, Script},
};
use anyhow::{anyhow, Result};
use dbus::{
    blocking::stdintf::org_freedesktop_dbus::PropertiesPropertiesChanged as Ppc,
    {ffidisp::Connection, message::SignalArgs},
};
use libsystemd::daemon::{self, NotifyState};
use log::{debug, info, warn};
use std::{path::PathBuf, sync::Arc};

/// A responder manages link event
#[derive(Debug)]
pub struct Broker {
    script_dir: PathBuf,
    timeout: u64,
    json: bool,
    verbose: u8,
    launcher: Launcher,
}

impl Broker {
    pub fn new<P>(script_dir: P, timeout: u64, json: bool, verbose: u8) -> Broker
    where
        P: Into<PathBuf>,
    {
        // Start script launcher
        let launcher = Launcher::new();

        Broker {
            script_dir: script_dir.into(),
            timeout,
            json,
            verbose,
            launcher,
        }
    }

    pub fn listen(&self) -> Result<()> {
        // Connect to DBus
        let connection = Connection::new_system()?;
        let matched_signal = Ppc::match_str(Some(&"org.freedesktop.network1".into()), None);
        debug!("Match Signal: {:?}", matched_signal);
        connection.add_match(&matched_signal)?;

        // Notify systemd that we are ready :)
        let _ = daemon::notify(false, &[NotifyState::Ready]);

        // Start DBus event loop
        info!("Start listening for link event.");
        loop {
            if let Some(msg) = connection.incoming(1000).next() {
                debug!("Link Message: {:?}", &msg);
                match LinkEvent::from_message(&msg) {
                    Ok(link_event) => {
                        debug!("Link Event: {:?}", link_event);
                        if let Err(e) = self.respond(&link_event) {
                            warn!("{}", e);
                        }
                    }
                    Err(e) => debug!("Error: {:?}", e),
                }
            }
        }
    }

    pub fn trigger_all(&self) -> Result<()> {
        let link_list = match Link::link_list() {
            Ok(link) => link,
            Err(err) => {
                return Err(anyhow!(
                    "Cannot trigger all interface, since no iface found, {}",
                    err
                ));
            }
        };

        info!("Start trigger all interfaces.");
        for (idx, link) in link_list.iter() {
            info!("Trigger on interface `{}`", link.iface);
            if let Ok(path) = dbus::Path::new(LinkEvent::index_to_dbus_path(*idx)) {
                // Create fake event
                let mut event = LinkEvent {
                    path,
                    state_type: StateType::OperationalState,
                    state: link.operational.clone(),
                };

                // 1: OperationalState
                if let Err(e) = self.respond(&event) {
                    warn!("{}", e);
                }

                // 2: AdministrativeState
                event.state_type = StateType::AdministrativeState;
                event.state = link.setup.clone();
                if let Err(e) = self.respond(&event) {
                    warn!("{}", e);
                }
            } else {
                // Display error and skip this interface.
                warn!("Cannot create D-Bus path from link index `{}`.", idx);
            }
        }

        Ok(())
    }

    fn respond(&self, event: &LinkEvent) -> Result<()> {
        // Convert link index to link name.
        let link_list = match Link::link_list() {
            Ok(link) => link,
            Err(err) => {
                return Err(anyhow!("Cannot get interface list, {}", err));
            }
        };

        let idx = event.index()?;
        let link = match link_list.get(&idx) {
            Some(l) => l,
            None => {
                return Err(anyhow!("Cannot find link index `{}`", idx));
            }
        };

        info!("Respond to '{}' event of '{}'", &event.state, &link.iface);

        // Get all scripts associated with current event
        let state_dir = format!("{}.d", event.state.to_string());
        let script_path = self.script_dir.join(state_dir);
        let scripts = match Script::get_scripts_in(&script_path, None, None) {
            Ok(s) => s,
            Err(e) => {
                return Err(anyhow!("{}", e));
            }
        };

        // Build script's arguments
        let mut args = Arguments::new();
        args.state(&event.state).iface(&link.iface);
        let shared_args = Arc::new(args);

        // Fetch status of iface
        let status = link.status()?;

        // Pack all event-related environments.
        let mut envs = Environments::new();
        envs.extract_from(event, link, status, self.json);
        let shared_envs = Arc::new(envs);

        // Push scripts with args + envs to launcher's queue.
        for mut s in scripts {
            s.args(Some(shared_args.clone()))
                .envs(Some(shared_envs.clone()))
                .timeout(self.timeout);
            if let Err(e) = self.launcher.add(s) {
                warn!("{}", e);
            }
        }

        Ok(())
    }
}
