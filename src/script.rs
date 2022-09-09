use crate::environment::Environments;
use anyhow::{anyhow, Result};
use std::{
    collections::HashMap,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    process::Command,
    sync::Arc,
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
    pub fn new<P>(path: P) -> Script
    where
        P: Into<PathBuf>,
    {
        let path = path.into();
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
        let path = self.path.to_str().unwrap();

        if self.no_wait {
            match self.execute_nowait() {
                Ok(_) => {
                    info!("Executed (nowait) {}", path);
                    Ok(())
                }
                Err(e) => {
                    warn!("Execute failed {}", path);
                    Err(e)
                }
            }
        } else {
            match self.execute_wait(self.timeout) {
                Ok(_) => {
                    info!("Executed {}", path);
                    Ok(())
                }
                Err(e) => {
                    warn!("Execute timeout {}", path);
                    Err(e)
                }
            }
        }
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
            &self.path.to_str().unwrap(),
            args[0],
            args[1]
        );
        match Command::new(&self.path).args(&args).envs(envs).spawn() {
            Ok(mut script) => {
                // Prevent zombie process by spawning thread to wait for the process to finish
                let script_path = self.path.to_str().unwrap().to_owned();
                std::thread::spawn(move || {
                    if let Err(err) = script.wait() {
                        warn!("{script_path} wasn't running, {err}");
                    } else {
                        info!("Finished {script_path}");
                    }
                });
                Ok(())
            }
            Err(e) => Err(anyhow!(
                "Execute `{}` failed: {}",
                &self.path.to_str().unwrap(),
                e
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
            &self.path.to_str().unwrap(),
            args[0],
            args[1]
        );
        let mut script = Command::new(&self.path)
            .args(&args)
            .envs(envs)
            .spawn()
            .unwrap();
        match script.wait_timeout(timeout).unwrap() {
            Some(_) => {
                info!("Finished {}", &self.path.to_str().unwrap());
                Ok(())
            }
            None => {
                // script hasn't exited yet
                script.kill().unwrap();
                Err(anyhow!(
                    "Execute `{}` is timeout: {secs} seconds",
                    &self.path.to_str().unwrap()
                ))
            }
        }
    }

    pub fn get_scripts_in<P>(path: P, uid: Option<u32>, gid: Option<u32>) -> Result<Vec<Script>>
    where
        P: AsRef<Path>,
    {
        let path = path.as_ref();

        // Path exists?
        if !path.exists() {
            return Err(anyhow!("`{}` does not exist", path.to_str().unwrap()));
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

            scripts.push(Script::new(entry.path()));
        }

        if scripts.is_empty() {
            return Err(anyhow!("No script in `{}`.", path.to_str().unwrap()));
        }

        Ok(scripts)
    }
}

#[cfg(test)]
mod tests {
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
        let script = Script::new("/etc/networkd/broker.d/carrier.d/00-script");
        assert!(!script.no_wait);

        // No-wait script
        let script = Script::new("/etc/networkd/broker.d/carrier.d/00-script-nowait");
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
        let scripts = Script::get_scripts_in(&carrier_d, Some(uid), Some(gid)).unwrap();
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
        let result = Script::get_scripts_in(&configuring_d, Some(uid), Some(gid));
        assert!(result.is_err());

        // No script for root in degraded.d
        let degraded_d = broker_root.join("degraded.d");
        let result = Script::get_scripts_in(&degraded_d, None, None);
        assert!(result.is_err());

        // No directory for routable state
        let routable_d = broker_root.join("routable.d");
        let result = Script::get_scripts_in(&routable_d, Some(uid), Some(gid));
        assert!(result.is_err());
    }
}
