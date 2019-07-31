use std::path::PathBuf;

use log::{debug, info, warn};

use dbus::{
    stdintf::org_freedesktop_dbus::PropertiesPropertiesChanged as PC,
    {BusType, Connection, SignalArgs},
};

use crate::{
    environment::{Environments, ScriptEnvironment},
    error::AppError,
    launcher::Launcher,
    link::{Link, LinkEvent},
    script::{Arguments, Script},
};

/// A responder manages link event
#[derive(Debug)]
pub struct Dispatcher {
    script_dir: PathBuf,
    run_startup_triggers: bool,
    timeout: u64,
    json: bool,
    verbose: u8,
}

impl Dispatcher {
    pub fn new<P>(
        script_dir: P,
        run_startup_triggers: bool,
        timeout: u64,
        json: bool,
        verbose: u8,
    ) -> Dispatcher
    where
        P: Into<PathBuf>,
    {
        Dispatcher {
            script_dir: script_dir.into(),
            run_startup_triggers,
            timeout,
            json,
            verbose,
        }
    }

    pub fn listen(&self) {
        // Start script launcher
        let launcher = Launcher::new();

        // Connect to DBus
        let connection = Connection::get_private(BusType::System).unwrap();
        let matched_signal = PC::match_str(Some(&"org.freedesktop.network1".into()), None);
        connection.add_match(&matched_signal).unwrap();

        // Start DBus event loop
        info!("Listening for link event...");
        loop {
            if let Some(msg) = connection.incoming(1000).next() {
                if let Ok(link_event) = LinkEvent::from_message(&msg) {
                    debug!("{:#?}", link_event);

                    // Convert link index to link name.
                    let links = match Link::link_list() {
                        Ok(l) => l,
                        Err(_) => {
                            warn!("Cannot get iface name");
                            continue;
                        }
                    };

                    let iface = match links.get(&link_event.index().unwrap()) {
                        Some(link) => &link.iface,
                        None => {
                            warn!("Cannot get iface name");
                            continue;
                        }
                    };

                    info!("Respond to '{}' event of '{}'", &link_event.state, &iface);

                    // Get all scripts associated with current event
                    let state_dir = format!("{}.d", link_event.state.to_string());
                    let script_path = self.script_dir.join(state_dir);
                    let scripts = match Script::get_scripts_in(&script_path, None, None) {
                        Ok(s) => s,
                        Err(AppError::NoPathFound) => {
                            info!("Path does not exist: {}", &script_path.to_str().unwrap());
                            continue;
                        }
                        Err(AppError::NoScriptFound) => {
                            info!("No script found in: {}", &script_path.to_str().unwrap());
                            continue;
                        }
                        Err(_) => continue,
                    };

                    // Push scripts with args + envs to launcher's queue.
                    for mut s in scripts {
                        // Build script's arguments
                        let mut args = Arguments::new();
                        args.state(&link_event.state).iface(iface);

                        // TODO: Pack all event-related environments.
                        let mut envs = Environments::new();
                        envs.add(ScriptEnvironment::DeviceIface, iface).add(
                            ScriptEnvironment::DispatcherAction,
                            link_event.state.to_string(),
                        );

                        s.args(args).envs(envs);
                        launcher.add(s);
                    }
                }
            }
        }
    }
}
