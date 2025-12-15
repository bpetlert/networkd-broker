use std::{
    io::{
        BufReader,
        Seek,
    },
    path::Path,
};

use networkd_broker::script::Script;

use crate::common::{
    IFACE,
    log_check::{
        next_log,
        setup_log,
        wait_for_thread,
    },
};

mod common;

// Wrong argument 1
#[test]
fn wrong_arg_1() {
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
        .set_arg0("wrong-arg0")
        .set_arg1(IFACE)
        .build();
    let ret = script.execute();
    wait_for_thread();
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
}
