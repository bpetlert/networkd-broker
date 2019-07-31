use std::{env, io::Write};

use structopt::StructOpt;

use env_logger::Builder;
use log::{debug, LevelFilter};

mod dispatcher;
mod environment;
mod error;
mod extcommand;
mod launcher;
mod link;
mod script;

use crate::dispatcher::Dispatcher;

#[derive(Debug, StructOpt)]
#[structopt(name = "networkd-dispatcherd", about = "networkd dispatcher daemon")]
struct Opt {
    /// Location under which to look for scripts
    #[structopt(
        short = "S",
        long = "script-dir",
        default_value = "/etc/networkd/dispatcher.d"
    )]
    script_dir: String,

    /// Generate events reflecting preexisting state and behavior on startup
    #[structopt(short = "T", long = "run-startup-triggers")]
    run_startup_triggers: bool,

    /// Script execution timeout in seconds
    #[structopt(short = "t", long = "timeout", default_value = "20")]
    timeout: u64,

    /// Pass JSON encoding of event and link status to script
    #[structopt(short = "j", long = "json")]
    json: bool,

    /// Increment verbosity level once per call
    /// [error, -v: warn, -vv: info, -vvv: debug, -vvvv: trace]
    #[structopt(short = "v", long = "verbose", parse(from_occurrences))]
    verbose: u8,
}

fn main() {
    let opt = Opt::from_args();
    let log_level = match opt.verbose {
        0 => LevelFilter::Error,
        1 => LevelFilter::Warn,
        2 => LevelFilter::Info,
        3 => LevelFilter::Debug,
        4 => LevelFilter::Trace,
        _ => LevelFilter::Trace,
    };
    Builder::new()
        .parse_filters(&env::var("NETWORKD_DISPATCHERD_LOG").unwrap_or_default())
        .format(|buf, record| writeln!(buf, "{}: {}", record.level(), record.args()))
        .filter(None, log_level)
        .init();
    debug!("Start with {:#?}", opt);

    let dispatcher = Dispatcher::new(
        opt.script_dir,
        opt.run_startup_triggers,
        opt.timeout,
        opt.json,
        opt.verbose,
    );
    debug!("Start dispatcher with {:#?}", dispatcher);
    dispatcher.listen();
}
