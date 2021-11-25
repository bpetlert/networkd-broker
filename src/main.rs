use anyhow::Result;
use log::{debug, error, info, warn, LevelFilter};
use std::{env, process};
use structopt::StructOpt;

mod args;
mod broker;
mod environment;
mod extcommand;
mod launcher;
mod link;
mod script;

use crate::{args::Arguments, broker::Broker};

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

    let mut log_builder = pretty_env_logger::formatted_builder();
    if let Ok(value) = env::var("RUST_LOG") {
        log_builder.parse_filters(&value);
    } else {
        log_builder.filter_level(log_level);
    }
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
        if let Err(err) = broker.trigger_all() {
            warn!("{}", err);
        }
    }

    broker.listen()
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
