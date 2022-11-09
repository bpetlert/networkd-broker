use crate::common::{
    log_check::{next_log, setup_log, wait_for_thread},
    STATE,
};
use networkd_broker::script::Script;
use std::{
    io::{BufReader, Seek},
    path::Path,
};

mod common;

// Wrong argument 2
#[test]
fn wrong_arg_2() {
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
        .set_arg1("wrong-arg1")
        .build();
    let ret = script.execute();
    wait_for_thread();
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
}
