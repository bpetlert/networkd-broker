use crate::common::{
    log_check::{next_log, setup_log, wait_for_thread},
    IFACE, STATE,
};
use networkd_broker::script::{EnvVar, Script};
use std::{
    io::{BufReader, Seek},
    path::Path,
};

mod common;

// Missing NWD_BROKER_ACTION environment variable
#[test]
fn missing_nwd_broker_action() {
    let mut log_file = setup_log();
    log_file.seek(std::io::SeekFrom::End(0)).unwrap();
    let mut reader = BufReader::new(log_file);

    let script_path = Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests",
        "/scripts",
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
}
