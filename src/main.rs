use anyhow::Result;
use std::process;
use structopt::StructOpt;
use tracing::{debug, error, info, warn};
use tracing_subscriber::EnvFilter;

mod args;
mod broker;
mod environment;
mod extcommand;
mod launcher;
mod link;
mod script;

use crate::{args::Arguments, broker::Broker};

fn init_log() -> Result<()> {
    let filter = match EnvFilter::try_from_env("RUST_LOG") {
        Ok(f) => f,
        Err(_) => EnvFilter::try_new("aur_thumbsup=warn")?,
    };
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .without_time()
        .try_init()
        .expect("Initialize tracing-subscriber");
    Ok(())
}

fn run_app() -> Result<()> {
    let arguments = Arguments::from_args();
    init_log().expect("Initialize logging");
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
