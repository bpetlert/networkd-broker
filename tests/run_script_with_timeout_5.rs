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

// Missing NWD_JSON environment variable
#[test]
fn missing_nwd_json() {
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
}
