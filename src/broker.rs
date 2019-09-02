use std::{path::PathBuf, sync::Arc};

use log::{debug, info, warn};

use dbus::{
    blocking::stdintf::org_freedesktop_dbus::PropertiesPropertiesChanged as PC,
    {ffidisp::Connection, message::SignalArgs},
};

use libsystemd::daemon::{self, NotifyState};

use crate::{
    environment::Environments,
    error::AppError,
    launcher::Launcher,
    link::{Link, LinkEvent, StateType},
    script::{Arguments, Script},
};

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

    pub fn listen(&self) {
        // Connect to DBus
        let connection = Connection::new_system().unwrap();
        let matched_signal = PC::match_str(Some(&"org.freedesktop.network1".into()), None);
        connection.add_match(&matched_signal).unwrap();

        // Notify systemd that we are ready :)
        let _ = daemon::notify(false, &[NotifyState::Ready]);

        // Start DBus event loop
        info!("Listening for link event...");
        loop {
            if let Some(msg) = connection.incoming(1000).next() {
                if let Ok(link_event) = LinkEvent::from_message(&msg) {
                    debug!("{:?}", link_event);
                    self.respond(&link_event);
                }
            }
        }
    }

    pub fn trigger_all(&self) {
        let link_list = match Link::link_list() {
            Ok(l) => l,
            Err(_) => {
                warn!("Cannot get iface name");
                return;
            }
        };

        for (idx, link) in link_list.iter() {
            // Create fake event
            let mut event = LinkEvent {
                path: dbus::Path::new(LinkEvent::index_to_dbus_path(*idx)).unwrap(),
                state_type: StateType::OperationalState,
                state: link.operational.clone(),
            };

            // 1: OperationalState
            self.respond(&event);

            // 2: AdministrativeState
            event.state_type = StateType::AdministrativeState;
            event.state = link.setup.clone();
            self.respond(&event);
        }
    }

    fn respond(&self, event: &LinkEvent) {
        // Convert link index to link name.
        let link_list = match Link::link_list() {
            Ok(l) => l,
            Err(_) => {
                warn!("Cannot get iface name");
                return;
            }
        };

        let link = match link_list.get(&event.index().unwrap()) {
            Some(l) => l,
            None => {
                warn!("Cannot get iface name");
                return;
            }
        };

        info!("Respond to '{}' event of '{}'", &event.state, &link.iface);

        // Get all scripts associated with current event
        let state_dir = format!("{}.d", event.state.to_string());
        let script_path = self.script_dir.join(state_dir);
        let scripts = match Script::get_scripts_in(&script_path, None, None) {
            Ok(s) => s,
            Err(AppError::NoPathFound) => {
                info!("Path does not exist: {}", &script_path.to_str().unwrap());
                return;
            }
            Err(AppError::NoScriptFound) => {
                info!("No script found in: {}", &script_path.to_str().unwrap());
                return;
            }
            Err(_) => return,
        };

        // Build script's arguments
        let mut args = Arguments::new();
        args.state(&event.state).iface(&link.iface);
        let shared_args = Arc::new(args);

        // Fetch status of iface
        let status = link.status().unwrap();

        // Pack all event-related environments.
        let mut envs = Environments::new();
        envs.extract_from(&event, link, status, self.json);
        let shared_envs = Arc::new(envs);

        // Push scripts with args + envs to launcher's queue.
        for mut s in scripts {
            s.args(Some(shared_args.clone()))
                .envs(Some(shared_envs.clone()))
                .timeout(self.timeout);
            self.launcher.add(s);
        }
    }
}
