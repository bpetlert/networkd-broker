use crate::environment::Environments;
use anyhow::{anyhow, bail, Result};
use std::{
    collections::HashMap, os::unix::fs::MetadataExt, path::PathBuf, process::Command, sync::Arc,
    time::Duration,
};
use tracing::{info, warn};
use wait_timeout::ChildExt;
use walkdir::WalkDir;

#[derive(Debug)]
pub struct ScriptArguments {
    pub state: String,
    pub iface: String,
}

impl ScriptArguments {
    pub fn new() -> ScriptArguments {
        ScriptArguments {
            state: String::new(),
            iface: String::new(),
        }
    }

    pub fn pack(&self) -> Vec<&String> {
        vec![&self.state, &self.iface]
    }
}

#[derive(Debug)]
pub struct Script {
    path: PathBuf,
    pub args: Option<Arc<ScriptArguments>>,
    pub envs: Option<Arc<Environments>>,
    no_wait: bool,
    pub timeout: u64,
}

impl Script {
    pub fn new(path: PathBuf) -> Script {
        let mut no_wait = false;
        let file_name = path.file_name().unwrap().to_str().unwrap();
        if file_name.ends_with("-nowait") {
            no_wait = true;
        }

        Script {
            path,
            args: None,
            envs: None,
            no_wait,
            timeout: 20,
        }
    }

    pub fn execute(&self) -> Result<()> {
        if self.no_wait {
            match self.execute_nowait() {
                Ok(_) => {
                    info!("Executed (nowait) {}", self.path.display());
                }
                Err(err) => {
                    warn!("{err}");
                }
            }
        } else {
            match self.execute_wait(self.timeout) {
                Ok(_) => {
                    info!("Executed {}", self.path.display());
                }
                Err(err) => {
                    warn!("{err}");
                }
            }
        }

        Ok(())
    }

    pub fn execute_nowait(&self) -> Result<()> {
        let args: Vec<&String> = match &self.args {
            Some(a) => a.pack(),
            None => Vec::new(),
        };

        let empty_envs: HashMap<String, String> = HashMap::new();
        let envs: &HashMap<String, String> = match &self.envs {
            Some(e) => e.pack(),
            None => &empty_envs,
        };

        info!(
            "Try to execute (nowait) {} {} {}",
            &self.path.display(),
            args[0],
            args[1]
        );
        match Command::new(&self.path).args(&args).envs(envs).spawn() {
            Ok(mut script) => {
                // Prevent zombie process by spawning thread to wait for the process to finish
                let script_path = self.path.clone();
                let arg0 = args[0].clone();
                let arg1 = args[1].clone();
                std::thread::spawn(move || match script.wait() {
                    Ok(exit_code) => {
                        info!(
                            "Finished {} {} {}, {exit_code}",
                            script_path.display(),
                            arg0,
                            arg1
                        );
                    }
                    Err(err) => {
                        warn!(
                            "{} {} {} wasn't running, {err}",
                            script_path.display(),
                            arg0,
                            arg1
                        );
                    }
                });
                Ok(())
            }
            Err(err) => Err(anyhow!(
                "Execute {} {} {} failed, {}",
                &self.path.display(),
                args[0],
                args[1],
                err
            )),
        }
    }

    pub fn execute_wait(&self, secs: u64) -> Result<()> {
        let args: Vec<&String> = match &self.args {
            Some(a) => a.pack(),
            None => Vec::new(),
        };

        let empty_envs: HashMap<String, String> = HashMap::new();
        let envs: &HashMap<String, String> = match &self.envs {
            Some(e) => e.pack(),
            None => &empty_envs,
        };

        let timeout = Duration::from_secs(secs);

        info!(
            "Try to execute {} {} {}",
            &self.path.display(),
            args[0],
            args[1]
        );
        let mut script = match Command::new(&self.path).args(&args).envs(envs).spawn() {
            Ok(script) => script,
            Err(err) => {
                bail!(
                    "Execute {} {} {} failed, {}",
                    &self.path.display(),
                    args[0],
                    args[1],
                    err
                );
            }
        };

        match script.wait_timeout(timeout)? {
            Some(exit_code) => {
                info!(
                    "Finished {} {} {}, {exit_code}",
                    &self.path.display(),
                    args[0],
                    args[1]
                );
                Ok(())
            }
            None => {
                // script hasn't exited yet
                script.kill()?;
                let exit_code = script.wait()?;
                Err(anyhow!(
                    "Execute timeout {} {} {}, >= {secs} seconds, {exit_code}",
                    &self.path.display(),
                    args[0],
                    args[1]
                ))
            }
        }
    }

    pub fn get_scripts_in(
        path: PathBuf,
        uid: Option<u32>,
        gid: Option<u32>,
    ) -> Result<Vec<Script>> {
        // Path exists?
        if !path.exists() {
            bail!("{} does not exist", path.display());
        }

        let uid = uid.unwrap_or(0);
        let gid = gid.unwrap_or(0);
        let mut scripts: Vec<Script> = Vec::new();
        for entry in WalkDir::new(&path)
            .min_depth(1)
            .max_depth(1)
            .sort_by(|a, b| a.file_name().cmp(b.file_name()))
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let metadata = entry.metadata().unwrap();
            if metadata.is_dir() {
                continue;
            }

            // Has exec access?
            if metadata.mode() & 0o111 != 0o111 {
                continue;
            }

            // Owned by root?
            if metadata.uid() != uid && metadata.gid() != gid {
                continue;
            }

            scripts.push(Script::new(entry.path().to_path_buf()));
        }

        if scripts.is_empty() {
            bail!("No script in {}.", path.display());
        }

        Ok(scripts)
    }
}

#[cfg(test)]
mod tests {
    use crate::environment::ScriptEnvironment;

    use super::*;
    use std::ffi::OsStr;
    use std::fs::{self, DirBuilder};
    use std::os::unix::fs::OpenOptionsExt;
    use tempfile::TempDir;
    use users::{get_current_gid, get_current_uid};

    fn setup_get_scripts_in() -> tempfile::TempDir {
        let temp_dir = TempDir::new().unwrap();
        assert!(temp_dir.path().to_owned().exists());

        // Create broker root directory
        let broker_root = temp_dir.path().join("etc/networkd/broker.d");
        DirBuilder::new()
            .recursive(true)
            .create(&broker_root)
            .unwrap();
        assert!(fs::metadata(&broker_root).unwrap().is_dir());

        // Create directory for each state
        for path in [
            "carrier.d",
            "degraded.d",
            "dormant.d",
            "no-carrier.d",
            "off.d",
            "routable.d",
        ]
        .iter()
        {
            let state_dir = &broker_root.join(path);
            DirBuilder::new().create(&state_dir).unwrap();
            assert!(fs::metadata(&state_dir).unwrap().is_dir());
        }

        // Create dummy scripts for current uid/gid
        let carrier_d = &broker_root.join("carrier.d");
        for (script, executable) in [
            ("00-executable", true),
            ("01-non-executable", false),
            ("05-executable-nowait", true),
            ("09-non-executable", false),
            ("10-executable", true),
        ]
        .iter()
        {
            let mode = if *executable { 0o555 } else { 0o444 };

            fs::OpenOptions::new()
                .create(true)
                .write(true)
                .mode(mode)
                .open(carrier_d.join(script))
                .unwrap();
        }

        // Create dummy scripts for current uid/gid
        let degraded_d = &broker_root.join("degraded.d");
        for (script, executable) in [
            ("00-non-root-executable", true),
            ("01-non-root-non-executable", false),
        ]
        .iter()
        {
            let mode = if *executable { 0o555 } else { 0o444 };

            fs::OpenOptions::new()
                .create(true)
                .write(true)
                .mode(mode)
                .open(degraded_d.join(script))
                .unwrap();
        }

        temp_dir
    }

    #[test]
    fn test_arguments_order() {
        let mut args = ScriptArguments::new();
        args.state = "routable".to_string();
        args.iface = "eth0".to_string();
        assert_eq!(args.pack(), vec!["routable", "eth0"]);
    }

    #[test]
    fn test_script_new() {
        // Normal script
        let script = Script::new(PathBuf::from("/etc/networkd/broker.d/carrier.d/00-script"));
        assert!(!script.no_wait);

        // No-wait script
        let script = Script::new(PathBuf::from(
            "/etc/networkd/broker.d/carrier.d/00-script-nowait",
        ));
        assert!(script.no_wait);
    }

    #[test]
    fn test_get_scripts_in() {
        let temp_dir = setup_get_scripts_in();
        let broker_root = temp_dir.path().join("etc/networkd/broker.d");
        let uid = get_current_uid();
        let gid = get_current_gid();

        // 3 scripts of current uid/gid for carrier state
        // 00-executable
        // 05-executable-nowait
        // 10-executable
        let carrier_d = broker_root.join("carrier.d");
        let scripts = Script::get_scripts_in(carrier_d, Some(uid), Some(gid)).unwrap();
        assert_eq!(scripts.len(), 3);
        assert_eq!(
            scripts[0].path.file_name(),
            Some(OsStr::new("00-executable"))
        );
        assert_eq!(
            scripts[1].path.file_name(),
            Some(OsStr::new("05-executable-nowait"))
        );
        assert_eq!(
            scripts[2].path.file_name(),
            Some(OsStr::new("10-executable"))
        );

        // No script for configuring state
        let configuring_d = broker_root.join("configuring.d");
        let result = Script::get_scripts_in(configuring_d, Some(uid), Some(gid));
        assert!(result.is_err());

        // No script for root in degraded.d
        let degraded_d = broker_root.join("degraded.d");
        let result = Script::get_scripts_in(degraded_d, None, None);
        assert!(result.is_err());

        // No directory for routable state
        let routable_d = broker_root.join("routable.d");
        let result = Script::get_scripts_in(routable_d, Some(uid), Some(gid));
        assert!(result.is_err());
    }

    #[test]
    fn test_script_execute() {
        let state = "routable".to_string();
        let iface = "wlp3s0".to_string();

        let mut args = ScriptArguments::new();
        args.state = state.clone();
        args.iface = iface.clone();
        let shared_args = Arc::new(args);

        let mut envs = Environments::new();
        envs.add(ScriptEnvironment::DeviceIface, iface)
            .add(ScriptEnvironment::BrokerAction, state)
            .add(ScriptEnvironment::Json, "".to_string());
        let shared_envs = Arc::new(envs);

        // Should pass, wait
        let mut script = Script::new(PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests",
            "/script-execute-test.sh"
        )));
        script.args = Some(shared_args.clone());
        script.envs = Some(shared_envs.clone());
        assert!(script.execute().is_ok(), "Should pass (wait)");

        // Should pass, no wait
        let mut script = Script::new(PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests",
            "/script-execute-test-nowait.sh"
        )));
        script.args = Some(shared_args.clone());
        script.envs = Some(shared_envs.clone());
        assert!(script.execute().is_ok(), "Should pass (nowait)");

        // no-such-file should not cause panic, wait
        let mut script = Script::new(PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests",
            "/no-such-file"
        )));
        script.args = Some(shared_args.clone());
        script.envs = Some(shared_envs.clone());
        let ret = script.execute();
        assert!(ret.is_ok(), "no-such-file should not cause panic (wait)");

        // no-such-file should not cause panic, nowait
        let mut script = Script::new(PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests",
            "/no-such-file-nowait"
        )));
        script.args = Some(shared_args.clone());
        script.envs = Some(shared_envs.clone());
        let ret = script.execute();
        assert!(ret.is_ok(), "no-such-file should not cause panic (nowait)");

        // Script failure should not cause panic, wait
        let mut script = Script::new(PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests",
            "/script-execute-test.sh"
        )));
        script.args = Some(shared_args.clone());
        script.envs = Some(shared_envs.clone());
        std::env::set_var("SCRIPT_FAILURE", "1");
        assert!(
            script.execute().is_ok(),
            "Script failure should not cause panic (wait)"
        );

        // Script failure should not cause panic, nowait
        let mut script = Script::new(PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests",
            "/script-execute-test-nowait.sh"
        )));
        script.args = Some(shared_args);
        script.envs = Some(shared_envs);
        std::env::set_var("SCRIPT_FAILURE", "1");
        assert!(
            script.execute().is_ok(),
            "Script failure should not cause panic (nowait)"
        );
    }
}
