use anyhow::{bail, Result};
use std::{
    collections::HashMap,
    fmt,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    process::Command,
    thread,
    time::Duration,
};
use tracing::{info, warn};
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

    pub fn build_from(
        path: PathBuf,
        uid: Option<u32>,
        gid: Option<u32>,
    ) -> Result<Vec<ScriptBuilder>> {
        // Path exists?
        if !path.exists() {
            bail!("{} does not exist", path.display());
        }

        let uid = uid.unwrap_or(0);
        let gid = gid.unwrap_or(0);
        let mut scripts: Vec<ScriptBuilder> = Vec::new();

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

            scripts.push(Script::builder().set_path(entry.path()));
        }

        if scripts.is_empty() {
            bail!("No script in {}.", path.display());
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
        {
            Ok(process) => {
                info!(
                    "Execute {script} {arg0} {arg1}",
                    script = &self.path.display(),
                    arg0 = self.args[0],
                    arg1 = self.args[1]
                );
                process
            }
            Err(err) => bail!(
                "Failed to execute {script} {arg0} {arg1}, {err}",
                script = &self.path.display(),
                arg0 = self.args[0],
                arg1 = self.args[1]
            ),
        };

        if let Some(timeout) = self.timeout {
            // Wait until child process to finish or timeout
            match process.wait_timeout(Duration::from_secs(timeout))? {
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
            thread::spawn(move || match process.wait() {
                Ok(exit_code) => info!(
                    "Finished executing {script} {arg0} {arg1}, {exit_code}",
                    script = &self.path.display(),
                    arg0 = self.args[0],
                    arg1 = self.args[1]
                ),
                Err(err) => warn!(
                    "{script} {arg0} {arg1} wasn't running, {err}",
                    script = &self.path.display(),
                    arg0 = self.args[0],
                    arg1 = self.args[1]
                ),
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
        fs::{self, DirBuilder, File},
        io::{BufRead, BufReader, Seek},
        os::unix::fs::OpenOptionsExt,
    };
    use tempfile::{NamedTempFile, TempDir};
    use tracing_subscriber::EnvFilter;
    use users::{get_current_gid, get_current_uid};

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
    fn build_new_script_from_dir() {
        let temp_dir = setup_script_dir();
        let broker_root = temp_dir.path().join("etc/networkd/broker.d");
        let uid = get_current_uid();
        let gid = get_current_gid();

        // 3 scripts of current uid/gid for carrier state
        // 00-executable
        // 05-executable-nowait
        // 10-executable
        let carrier_d = broker_root.join("carrier.d");
        let scripts = ScriptBuilder::build_from(carrier_d, Some(uid), Some(gid)).unwrap();
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
        let result = ScriptBuilder::build_from(configuring_d, Some(uid), Some(gid));
        assert!(result.is_err());

        // No script for root in degraded.d
        let degraded_d = broker_root.join("degraded.d");
        let result = ScriptBuilder::build_from(degraded_d, None, None);
        assert!(result.is_err());

        // No directory for routable state
        let routable_d = broker_root.join("routable.d");
        let result = ScriptBuilder::build_from(routable_d, Some(uid), Some(gid));
        assert!(result.is_err());
    }

    #[test]
    /// Script execution failure should not cause program to panic.
    ///
    /// Use log entries to verify script execution.
    fn execute_script() {
        let log_file = setup_log();
        execute_script_with_timeout(log_file.reopen().unwrap());
        execute_script_without_timeout(log_file.reopen().unwrap());
    }

    fn execute_script_with_timeout(mut log_file: File) {
        log_file.seek(std::io::SeekFrom::End(0)).unwrap();
        let mut reader = BufReader::new(log_file);

        const STATE: &str = "routable";
        const IFACE: &str = "wlp3s0";

        fn next_log(reader: &mut BufReader<File>) -> String {
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            line
        }

        // Wrong argument 1
        let script_path = Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests",
            "/script-execute-test.sh"
        ));
        let script = Script::builder()
            .set_path(script_path)
            .set_arg0("wrong-arg0")
            .set_arg1(IFACE)
            .build();
        let ret = script.execute();
        assert!(ret.is_ok(), "Wrong argument 1");
        assert_eq!(
            next_log(&mut reader),
            format!(
                " INFO networkd_broker::script: Execute {} wrong-arg0 {IFACE}\n",
                script_path.display()
            )
        );
        assert_eq!(
            next_log(&mut reader),
            format!(
                " INFO networkd_broker::script: Finished executing {} wrong-arg0 {IFACE}, exit status: 52\n",
                script_path.display()
            )
        );

        // Wrong argument 2
        let script_path = Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests",
            "/script-execute-test.sh"
        ));
        let script = Script::builder()
            .set_path(script_path)
            .set_arg0(STATE)
            .set_arg1("wrong-arg1")
            .build();
        let ret = script.execute();
        assert!(ret.is_ok(), "Wrong argument 2");
        assert_eq!(
            next_log(&mut reader),
            format!(
                " INFO networkd_broker::script: Execute {} {STATE} wrong-arg1\n",
                script_path.display()
            )
        );
        assert_eq!(
            next_log(&mut reader),
            format!(
                " INFO networkd_broker::script: Finished executing {} {STATE} wrong-arg1, exit status: 53\n",
                script_path.display()
            )
        );

        // Missing NWD_DEVICE_IFACE environment variable
        let script_path = Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests",
            "/script-execute-test.sh"
        ));
        let script = Script::builder()
            .set_path(script_path)
            .set_arg0(STATE)
            .set_arg1(IFACE)
            .build();
        let ret = script.execute();
        assert!(ret.is_ok(), "Missing NWD_DEVICE_IFACE environment variable");
        assert_eq!(
            next_log(&mut reader),
            format!(
                " INFO networkd_broker::script: Execute {} {STATE} {IFACE}\n",
                script_path.display()
            )
        );
        assert_eq!(
            next_log(&mut reader),
            format!(
                " INFO networkd_broker::script: Finished executing {} {STATE} {IFACE}, exit status: 54\n",
                script_path.display()
            )
        );

        // Missing NWD_BROKER_ACTION environment variable
        let script_path = Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests",
            "/script-execute-test.sh"
        ));
        let script = Script::builder()
            .set_path(script_path)
            .set_arg0(STATE)
            .set_arg1(IFACE)
            .add_env(EnvVar::DeviceIface(IFACE.to_string()))
            .build();
        let ret = script.execute();
        assert!(
            ret.is_ok(),
            "Missing NWD_BROKER_ACTION environment variable"
        );
        assert_eq!(
            next_log(&mut reader),
            format!(
                " INFO networkd_broker::script: Execute {} {STATE} {IFACE}\n",
                script_path.display()
            )
        );
        assert_eq!(
            next_log(&mut reader),
            format!(
                " INFO networkd_broker::script: Finished executing {} {STATE} {IFACE}, exit status: 55\n",
                script_path.display()
            )
        );

        // Missing NWD_JSON environment variable
        let script_path = Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests",
            "/script-execute-test.sh"
        ));
        let script = Script::builder()
            .set_path(script_path)
            .set_arg0(STATE)
            .set_arg1(IFACE)
            .add_env(EnvVar::DeviceIface(IFACE.to_string()))
            .add_env(EnvVar::BrokerAction(STATE.to_string()))
            .build();
        let ret = script.execute();
        assert!(ret.is_ok(), "Missing NWD_JSON environment variable");
        assert_eq!(
            next_log(&mut reader),
            format!(
                " INFO networkd_broker::script: Execute {} {STATE} {IFACE}\n",
                script_path.display()
            )
        );
        assert_eq!(
            next_log(&mut reader),
            format!(
                " INFO networkd_broker::script: Finished executing {} {STATE} {IFACE}, exit status: 56\n",
                script_path.display()
            )
        );

        // Script failed
        let script_path = Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests",
            "/script-execute-test.sh"
        ));
        let script = Script::builder()
            .set_path(script_path)
            .set_arg0(STATE)
            .set_arg1(IFACE)
            .add_env(EnvVar::DeviceIface(IFACE.to_string()))
            .add_env(EnvVar::BrokerAction(STATE.to_string()))
            .add_env(EnvVar::Json("".to_string()))
            .add_env(EnvVar::Custom {
                key: "SCRIPT_FAILURE".to_string(),
                value: "1".to_string(),
            })
            .build();
        let ret = script.execute();
        assert!(ret.is_ok(), "Script failed");
        assert_eq!(
            next_log(&mut reader),
            format!(
                " INFO networkd_broker::script: Execute {} {STATE} {IFACE}\n",
                script_path.display()
            )
        );
        assert_eq!(
            next_log(&mut reader),
            format!(
                " INFO networkd_broker::script: Finished executing {} {STATE} {IFACE}, exit status: 2\n",
                script_path.display()
            )
        );

        // Script is not exist.
        let script_path = Path::new("/tmp/not-exist-script.sh");
        let script = Script::builder()
            .set_path(script_path)
            .set_arg0(STATE)
            .set_arg1(IFACE)
            .add_env(EnvVar::DeviceIface(IFACE.to_string()))
            .add_env(EnvVar::BrokerAction(STATE.to_string()))
            .add_env(EnvVar::Json("".to_string()))
            .build();
        let ret = script.execute();
        assert!(ret.is_err(), "Script is not exist");
        warn!("{}", ret.unwrap_err());
        assert_eq!(
            next_log(&mut reader),
            format!(
                " WARN networkd_broker::script::tests: Failed to execute {} routable wlp3s0, No such file or directory (os error 2)\n",
                script_path.display()
            )
        );

        // Script execution timeout.
        let script_path = Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests",
            "/script-execute-test.sh"
        ));
        let script = Script::builder()
            .set_path(script_path)
            .set_arg0(STATE)
            .set_arg1(IFACE)
            .add_env(EnvVar::DeviceIface(IFACE.to_string()))
            .add_env(EnvVar::BrokerAction(STATE.to_string()))
            .add_env(EnvVar::Json("".to_string()))
            .add_env(EnvVar::Custom {
                key: "SCRIPT_FAILURE".to_string(),
                value: "2".to_string(),
            })
            .set_default_timeout(2)
            .build();
        let ret = script.execute();
        assert!(ret.is_err(), "Script execution timeout");
        warn!("{}", ret.unwrap_err());
        assert_eq!(
            next_log(&mut reader),
            format!(
                " INFO networkd_broker::script: Execute {} routable wlp3s0\n",
                script_path.display()
            )
        );
        assert_eq!(
            next_log(&mut reader),
            format!(
                " WARN networkd_broker::script::tests: Execute timeout {} routable wlp3s0, >= 2 seconds, signal: 9 (SIGKILL)\n",
                script_path.display()
            )
        );
    }

    fn execute_script_without_timeout(mut log_file: File) {
        log_file.seek(std::io::SeekFrom::End(0)).unwrap();
        let mut reader = BufReader::new(log_file);

        const STATE: &str = "routable";
        const IFACE: &str = "wlp3s0";

        fn next_log(reader: &mut BufReader<File>) -> String {
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            line
        }

        fn wait_for_thread() {
            thread::sleep(std::time::Duration::from_secs(2));
        }

        println!("{}", next_log(&mut reader));
        println!("{}", next_log(&mut reader));
        println!("{}", next_log(&mut reader));

        // Wrong argument 1
        let script_path = Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests",
            "/script-execute-test-nowait.sh"
        ));
        let script = Script::builder()
            .set_path(script_path)
            .set_arg0("wrong-arg0")
            .set_arg1(IFACE)
            .build();
        let ret = script.execute();
        assert!(ret.is_ok(), "Wrong argument 1");
        wait_for_thread();
        assert_eq!(
            next_log(&mut reader),
            format!(
                " INFO networkd_broker::script: Execute {} wrong-arg0 {IFACE}\n",
                script_path.display()
            )
        );
        assert_eq!(
            next_log(&mut reader),
            format!(
                " INFO networkd_broker::script: Finished executing {} wrong-arg0 {IFACE}, exit status: 52\n",
                script_path.display()
            )
        );

        // Wrong argument 2
        let script_path = Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests",
            "/script-execute-test-nowait.sh"
        ));
        let script = Script::builder()
            .set_path(script_path)
            .set_arg0(STATE)
            .set_arg1("wrong-arg1")
            .build();
        let ret = script.execute();
        assert!(ret.is_ok(), "Wrong argument 2");
        wait_for_thread();
        assert_eq!(
            next_log(&mut reader),
            format!(
                " INFO networkd_broker::script: Execute {} {STATE} wrong-arg1\n",
                script_path.display()
            )
        );
        assert_eq!(
            next_log(&mut reader),
            format!(
                " INFO networkd_broker::script: Finished executing {} {STATE} wrong-arg1, exit status: 53\n",
                script_path.display()
            )
        );

        // Missing NWD_DEVICE_IFACE environment variable
        let script_path = Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests",
            "/script-execute-test-nowait.sh"
        ));
        let script = Script::builder()
            .set_path(script_path)
            .set_arg0(STATE)
            .set_arg1(IFACE)
            .build();
        let ret = script.execute();
        wait_for_thread();
        assert!(ret.is_ok(), "Missing NWD_DEVICE_IFACE environment variable");
        assert_eq!(
            next_log(&mut reader),
            format!(
                " INFO networkd_broker::script: Execute {} {STATE} {IFACE}\n",
                script_path.display()
            )
        );
        assert_eq!(
            next_log(&mut reader),
            format!(
                " INFO networkd_broker::script: Finished executing {} {STATE} {IFACE}, exit status: 54\n",
                script_path.display()
            )
        );

        // Missing NWD_BROKER_ACTION environment variable
        let script_path = Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests",
            "/script-execute-test-nowait.sh"
        ));
        let script = Script::builder()
            .set_path(script_path)
            .set_arg0(STATE)
            .set_arg1(IFACE)
            .add_env(EnvVar::DeviceIface(IFACE.to_string()))
            .build();
        let ret = script.execute();
        wait_for_thread();
        assert!(
            ret.is_ok(),
            "Missing NWD_BROKER_ACTION environment variable"
        );
        assert_eq!(
            next_log(&mut reader),
            format!(
                " INFO networkd_broker::script: Execute {} {STATE} {IFACE}\n",
                script_path.display()
            )
        );
        assert_eq!(
            next_log(&mut reader),
            format!(
                " INFO networkd_broker::script: Finished executing {} {STATE} {IFACE}, exit status: 55\n",
                script_path.display()
            )
        );

        // Missing NWD_JSON environment variable
        let script_path = Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests",
            "/script-execute-test-nowait.sh"
        ));
        let script = Script::builder()
            .set_path(script_path)
            .set_arg0(STATE)
            .set_arg1(IFACE)
            .add_env(EnvVar::DeviceIface(IFACE.to_string()))
            .add_env(EnvVar::BrokerAction(STATE.to_string()))
            .build();
        let ret = script.execute();
        wait_for_thread();
        assert!(ret.is_ok(), "Missing NWD_JSON environment variable");
        assert_eq!(
            next_log(&mut reader),
            format!(
                " INFO networkd_broker::script: Execute {} {STATE} {IFACE}\n",
                script_path.display()
            )
        );
        assert_eq!(
            next_log(&mut reader),
            format!(
                " INFO networkd_broker::script: Finished executing {} {STATE} {IFACE}, exit status: 56\n",
                script_path.display()
            )
        );

        // Script failed
        let script_path = Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests",
            "/script-execute-test-nowait.sh"
        ));
        let script = Script::builder()
            .set_path(script_path)
            .set_arg0(STATE)
            .set_arg1(IFACE)
            .add_env(EnvVar::DeviceIface(IFACE.to_string()))
            .add_env(EnvVar::BrokerAction(STATE.to_string()))
            .add_env(EnvVar::Json("".to_string()))
            .add_env(EnvVar::Custom {
                key: "SCRIPT_FAILURE".to_string(),
                value: "1".to_string(),
            })
            .build();
        let ret = script.execute();
        wait_for_thread();
        assert!(ret.is_ok(), "Script failed");
        assert_eq!(
            next_log(&mut reader),
            format!(
                " INFO networkd_broker::script: Execute {} {STATE} {IFACE}\n",
                script_path.display()
            )
        );
        assert_eq!(
            next_log(&mut reader),
            format!(
                " INFO networkd_broker::script: Finished executing {} {STATE} {IFACE}, exit status: 2\n",
                script_path.display()
            )
        );

        // Script is not exist.
        let script_path = Path::new("/tmp/not-exist-script-nowait.sh");
        let script = Script::builder()
            .set_path(script_path)
            .set_arg0(STATE)
            .set_arg1(IFACE)
            .add_env(EnvVar::DeviceIface(IFACE.to_string()))
            .add_env(EnvVar::BrokerAction(STATE.to_string()))
            .add_env(EnvVar::Json("".to_string()))
            .build();
        let ret = script.execute();
        assert!(ret.is_err(), "Script is not exist");
        warn!("{}", ret.unwrap_err());
        assert_eq!(
            next_log(&mut reader),
            format!(
                " WARN networkd_broker::script::tests: Failed to execute {} routable wlp3s0, No such file or directory (os error 2)\n",
                script_path.display()
            )
        );

        // Script execution nowait.
        let script_path = Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests",
            "/script-execute-test-nowait.sh"
        ));
        let script = Script::builder()
            .set_path(script_path)
            .set_arg0(STATE)
            .set_arg1(IFACE)
            .add_env(EnvVar::DeviceIface(IFACE.to_string()))
            .add_env(EnvVar::BrokerAction(STATE.to_string()))
            .add_env(EnvVar::Json("".to_string()))
            .add_env(EnvVar::Custom {
                key: "SCRIPT_FAILURE".to_string(),
                value: "3".to_string(),
            })
            .build();
        let ret = script.execute();
        assert!(ret.is_ok(), "Script execution nowait");
        assert_eq!(
            next_log(&mut reader),
            format!(
                " INFO networkd_broker::script: Execute {} routable wlp3s0\n",
                script_path.display()
            )
        );
        thread::sleep(std::time::Duration::from_secs(3));
        assert_eq!(
            next_log(&mut reader),
            format!(
                " INFO networkd_broker::script: Finished executing {} routable wlp3s0, exit status: 0\n",
                script_path.display()
            )
        );
    }

    fn setup_log() -> NamedTempFile {
        let log_file = NamedTempFile::new().unwrap();
        // println!("{}", log_file.path().display());
        tracing_subscriber::fmt()
            .with_env_filter(EnvFilter::new("networkd_broker=debug"))
            .without_time()
            .with_writer(log_file.reopen().unwrap())
            .init();
        log_file
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
