use anyhow::Result;
use log::LevelFilter;
use log::{debug, error, info};
use std::env;
use std::process;
use structopt::StructOpt;

mod args;
mod broker;
mod environment;
mod extcommand;
mod launcher;
mod link;
mod script;

use crate::args::Arguments;
use crate::broker::Broker;

fn run_app() -> Result<()> {
    let arguments = Arguments::from_args();
    let log_level = match arguments.verbose {
        0 => LevelFilter::Error,
        1 => LevelFilter::Warn,
        2 => LevelFilter::Info,
        3 => LevelFilter::Debug,
        4 => LevelFilter::Trace,
        _ => LevelFilter::Trace,
    };
    pretty_env_logger::formatted_builder()
        .parse_filters(&env::var("NETWORKD_BROKER_LOG").unwrap_or_default())
        .filter(None, log_level)
        .init();
    debug!("Run with {:?}", arguments);

    let broker = Broker::new(
        arguments.script_dir,
        arguments.timeout,
        arguments.json,
        arguments.verbose,
    );
    debug!("Start event broker with {:?}", broker);

    if arguments.run_startup_triggers {
        info!("Execute all scripts for the current state for each interface");
        broker.trigger_all();
    }

    broker.listen();

    Ok(())
}

fn main() {
    process::exit(match run_app() {
        Ok(_) => 0,
        Err(err) => {
            error!("{}", err);
            1
        }
    });
}
