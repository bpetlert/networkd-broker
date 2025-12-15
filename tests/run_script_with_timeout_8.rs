use std::{
    io::{
        BufReader,
        Seek,
    },
    path::Path,
};

use networkd_broker::script::{
    EnvVar,
    Script,
};

use crate::common::{
    IFACE,
    STATE,
    log_check::{
        next_log,
        setup_log,
    },
};

mod common;

// Script execution timeout.
#[test]
fn script_execution_timeout() {
    let mut log_file = setup_log();
    log_file.seek(std::io::SeekFrom::End(0)).unwrap();
    let mut reader = BufReader::new(log_file);

    let script_path = Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests",
        "/scripts",
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
            key: "SCRIPT_TEST_CASE".to_string(),
            value: "2".to_string(),
        })
        .set_default_timeout(2)
        .build();
    let ret = script.execute();
    assert_eq!(
        next_log(&mut reader),
        format!(
            " INFO networkd_broker::script: Execute {} routable wlp3s0\n",
            script_path.display()
        )
    );
    assert!(ret.is_err(), "Script execution timeout");
    assert_eq!(
        format!("{}", ret.unwrap_err().root_cause()),
        format!(
            "Execute timeout {} routable wlp3s0, >= 2 seconds, signal: 9 (SIGKILL)",
            script_path.display()
        )
    );
    assert_eq!(next_log(&mut reader), "");
}
