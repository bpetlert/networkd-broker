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

// Script is not exist.
#[test]
fn script_is_not_exist() {
    let mut log_file = setup_log();
    log_file.seek(std::io::SeekFrom::End(0)).unwrap();
    let mut reader = BufReader::new(log_file);

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
    assert_eq!(
        format!("{}", ret.unwrap_err().root_cause()),
        format!(
            "Failed to execute {} routable wlp3s0: No such file or directory (os error 2)",
            script_path.display()
        )
    );
    assert_eq!(next_log(&mut reader), "");
}
