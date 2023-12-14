use std::{
    collections::HashMap,
    fmt,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    process::Command,
    thread,
    time::Duration,
};

use anyhow::{bail, Context, Result};
use tracing::{debug, info, warn};
use wait_timeout::ChildExt;
use walkdir::WalkDir;

pub const DEFAULT_TIMEOUT: u64 = 20; // seconds

#[derive(Debug)]
pub enum EnvVar {
    DeviceIface(String),
    BrokerAction(String),
    Json(String),

    #[allow(dead_code)]
    Custom {
        key: String,
        value: String,
    },
}

impl fmt::Display for EnvVar {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            EnvVar::DeviceIface(_) => write!(f, "NWD_DEVICE_IFACE"),
            EnvVar::BrokerAction(_) => write!(f, "NWD_BROKER_ACTION"),
            EnvVar::Json(_) => write!(f, "NWD_JSON"),
            EnvVar::Custom { key, value: _ } => write!(f, "NWD_{key}"),
        }
    }
}

#[derive(Debug)]
pub struct ScriptBuilder {
    path: PathBuf,

    /// state
    arg0: String,

    /// iface
    arg1: String,

    envs: HashMap<String, String>,

    default_timeout: u64,
}

impl ScriptBuilder {
    pub fn set_path(mut self, path: &Path) -> Self {
        self.path = path.to_path_buf();
        self
    }

    pub fn set_arg0(mut self, state: &str) -> Self {
        self.arg0 = state.to_string();
        self
    }

    pub fn set_arg1(mut self, iface: &str) -> Self {
        self.arg1 = iface.to_string();
        self
    }

    pub fn add_env(mut self, env_var: EnvVar) -> Self {
        let value = match &env_var {
            EnvVar::DeviceIface(value)
            | EnvVar::BrokerAction(value)
            | EnvVar::Json(value)
            | EnvVar::Custom { key: _, value } => value,
        };

        self.envs.insert(env_var.to_string(), value.to_string());
        self
    }

    pub fn set_default_timeout(mut self, timeout: u64) -> Self {
        self.default_timeout = timeout;
        self
    }

    pub fn build(self) -> Script {
        let timeout = if ScriptBuilder::should_run_nowait(&self.path) {
            None
        } else {
            Some(self.default_timeout)
        };

        Script {
            path: self.path,
            args: vec![self.arg0, self.arg1],
            envs: self.envs,
            timeout,
        }
    }

    /// Get executable scripts from a path
    ///
    /// * `uid` - Acceptable user ID of a script. Default is 0 (root)
    /// * `gid` - Acceptable group ID of a script. Default is 0 (root)
    ///
    pub fn build_from(
        path: &Path,
        uid: Option<u32>,
        gid: Option<u32>,
    ) -> Result<Vec<ScriptBuilder>> {
        let mut scripts: Vec<ScriptBuilder> = Vec::new();

        if !path.exists() {
            debug!("`{}` does not exist", path.display());
            return Ok(scripts);
        }

        let uid = uid.unwrap_or(0); // Default is UID of root
        let gid = gid.unwrap_or(0); // Default is GID of root

        for entry in WalkDir::new(path)
            .min_depth(1)
            .max_depth(1)
            .follow_links(true)
            .sort_by(|a, b| a.file_name().cmp(b.file_name()))
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let metadata = match entry
                .metadata()
                .with_context(|| format!("Failed to get metadata of `{}`", entry.path().display()))
            {
                Ok(m) => m,
                Err(err) => {
                    warn!("{err:#}");
                    continue;
                }
            };

            if metadata.is_dir() {
                debug!("Ignore `{}`. It is a directory.", entry.path().display());
                continue;
            }

            // Has at least 500 for file mode
            if metadata.mode() & 0o500 != 0o500 {
                warn!("Ignore `{}`. It is not executable.", entry.path().display());
                continue;
            }

            if metadata.uid() != uid {
                warn!(
                    "Ignore `{}`. It is not owned by uid {uid}",
                    entry.path().display()
                );
                continue;
            }

            if metadata.gid() != gid {
                warn!(
                    "Ignore `{}`. It is not owned by gid {gid}",
                    entry.path().display()
                );
                continue;
            }

            scripts.push(Script::builder().set_path(entry.path()));
        }

        if scripts.is_empty() {
            debug!("No script in `{}`", path.display());
        }

        Ok(scripts)
    }

    fn should_run_nowait(path: &Path) -> bool {
        path.file_stem()
            .unwrap()
            .to_str()
            .unwrap()
            .ends_with("-nowait")
    }
}

#[derive(Debug)]
pub struct Script {
    path: PathBuf,
    args: Vec<String>,
    envs: HashMap<String, String>,
    timeout: Option<u64>,
}

impl Script {
    pub fn builder() -> ScriptBuilder {
        ScriptBuilder {
            path: PathBuf::new(),
            arg0: String::new(),
            arg1: String::new(),
            envs: HashMap::new(),
            default_timeout: DEFAULT_TIMEOUT,
        }
    }

    pub fn execute(self) -> Result<()> {
        let mut process = match Command::new(&self.path)
            .args(self.args.clone())
            .envs(self.envs)
            .spawn()
            .with_context(|| {
                format!(
                    "Failed to execute {script} {arg0} {arg1}",
                    script = &self.path.display(),
                    arg0 = self.args[0],
                    arg1 = self.args[1]
                )
            }) {
            Ok(process) => {
                info!(
                    "Execute {script} {arg0} {arg1}",
                    script = &self.path.display(),
                    arg0 = self.args[0],
                    arg1 = self.args[1]
                );
                process
            }
            Err(err) => bail!("{err:#}"),
        };

        if let Some(timeout) = self.timeout {
            match process
                .wait_timeout(Duration::from_secs(timeout))
                .context("Failed to wait until child process to finish or timeout")?
            {
                Some(exit_code) => {
                    info!(
                        "Finished executing {script} {arg0} {arg1}, {exit_code}",
                        script = &self.path.display(),
                        arg0 = self.args[0],
                        arg1 = self.args[1]
                    );
                    return Ok(());
                }
                None => {
                    process.kill()?;
                    let exit_code = process.wait()?;
                    bail!(
                        "Execute timeout {script} {arg0} {arg1}, >= {timeout} seconds, {exit_code}",
                        script = &self.path.display(),
                        arg0 = self.args[0],
                        arg1 = self.args[1]
                    );
                }
            }
        } else {
            // Use thread to wait for child process' return code.
            thread::spawn(move || {
                match process
                    .wait()
                    .context("Failed to wait until child process to finish")
                {
                    Ok(exit_code) => info!(
                        "Finished executing {script} {arg0} {arg1}, {exit_code}",
                        script = &self.path.display(),
                        arg0 = self.args[0],
                        arg1 = self.args[1]
                    ),
                    Err(err) => warn!(
                        "{script} {arg0} {arg1} wasn't running: {err:#}",
                        script = &self.path.display(),
                        arg0 = self.args[0],
                        arg1 = self.args[1]
                    ),
                }
            });
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        ffi::OsStr,
        fs::{self, DirBuilder},
        ops::Deref,
        os::unix::fs::OpenOptionsExt,
    };
    use sysinfo::{get_current_pid, ProcessExt, System, SystemExt, UserExt};
    use tempfile::TempDir;

    #[test]
    fn should_run_nowait() {
        assert!(!ScriptBuilder::should_run_nowait(Path::new(
            "/a/b/c/script.sh"
        )));

        assert!(!ScriptBuilder::should_run_nowait(Path::new(
            "/a/b/c/script"
        )));

        assert!(ScriptBuilder::should_run_nowait(Path::new(
            "/a/b/c/script-nowait.sh"
        )));

        assert!(ScriptBuilder::should_run_nowait(Path::new(
            "/a/b/c/script-nowait"
        )));

        assert!(!ScriptBuilder::should_run_nowait(Path::new(
            "/a/b/c/script.sh-nowait"
        )));

        assert!(!ScriptBuilder::should_run_nowait(Path::new(
            "/a/b/c/script.-nowait"
        )));

        assert!(ScriptBuilder::should_run_nowait(Path::new(
            "/a/b/c/script.-nowait.sh"
        )));
    }

    #[test]
    fn build_new_script() {
        // Script without extension
        let script = Script::builder()
            .set_path(Path::new("/etc/networkd/broker.d/carrier.d/00-script"))
            .build();
        assert_eq!(script.timeout, Some(DEFAULT_TIMEOUT));

        // Script with extension
        let script = Script::builder()
            .set_path(Path::new("/etc/networkd/broker.d/carrier.d/00-script.sh"))
            .build();
        assert_eq!(script.timeout, Some(DEFAULT_TIMEOUT));

        // No-wait script without extension
        let script = Script::builder()
            .set_path(Path::new(
                "/etc/networkd/broker.d/carrier.d/00-script-nowait",
            ))
            .build();
        assert_eq!(script.timeout, None);

        // No-wait script with extension
        let script = Script::builder()
            .set_path(Path::new(
                "/etc/networkd/broker.d/carrier.d/00-script-nowait.sh",
            ))
            .build();
        assert_eq!(script.timeout, None);
    }

    #[test]
    fn test_build_new_script_from_dir() {
        let temp_dir = setup_script_dir();
        let broker_root = temp_dir.path().join("etc/networkd/broker.d");
        let pid = get_current_pid().unwrap();
        let system: System = System::new_all();
        let p = system.process(pid).unwrap();
        let user = system.get_user_by_id(p.user_id().unwrap()).unwrap();
        let uid = user.id();
        let uid: u32 = *uid.deref();
        let gid: u32 = *user.group_id().deref();

        // 3 scripts of current uid/gid for carrier state
        // 00-executable
        // 05-executable-nowait
        // 10-executable
        let carrier_d = broker_root.join("carrier.d");
        let scripts = ScriptBuilder::build_from(&carrier_d, Some(uid), Some(gid)).unwrap();
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
        let result = ScriptBuilder::build_from(&configuring_d, Some(uid), Some(gid));
        assert!(result.is_err());

        // No script for root in degraded.d
        let degraded_d = broker_root.join("degraded.d");
        let result = ScriptBuilder::build_from(&degraded_d, None, None);
        assert!(result.is_err());

        // No directory for routable state
        let routable_d = broker_root.join("routable.d");
        let result = ScriptBuilder::build_from(&routable_d, Some(uid), Some(gid));
        assert!(result.is_err());
    }

    fn setup_script_dir() -> tempfile::TempDir {
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
            DirBuilder::new().create(state_dir).unwrap();
            assert!(fs::metadata(state_dir).unwrap().is_dir());
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
}
