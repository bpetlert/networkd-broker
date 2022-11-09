use std::{
    fs::File,
    io::{BufRead, BufReader},
    thread,
};
use tempfile::NamedTempFile;
use tracing_subscriber::EnvFilter;

pub(crate) fn setup_log() -> File {
    let log_file = NamedTempFile::new().unwrap();
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new("networkd_broker=info"))
        .without_time()
        .with_writer(log_file.reopen().unwrap())
        .init();
    log_file.reopen().unwrap()
}

pub(crate) fn next_log(reader: &mut BufReader<File>) -> String {
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    line
}

#[allow(dead_code)]
pub(crate) fn wait_for_thread() {
    thread::sleep(std::time::Duration::from_secs(2));
}
