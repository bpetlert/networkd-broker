use anyhow::Result;
use async_std::task;
use clap::Parser;
use std::process;
use tracing::{debug, error, info, warn};
use tracing_subscriber::EnvFilter;

mod args;
mod broker;
mod dbus_interface;
mod environment;
mod launcher;
mod link;
mod script;

use crate::{args::Arguments, broker::Broker};

fn init_log() -> Result<()> {
    let filter = match EnvFilter::try_from_env("RUST_LOG") {
        Ok(f) => f,
        Err(_) => EnvFilter::try_new("networkd_broker=warn")?,
    };
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .without_time()
        .try_init()
        .expect("Initialize tracing-subscriber");
    Ok(())
}

async fn run_app() -> anyhow::Result<()> {
    let arguments = Arguments::parse();
    init_log().expect("Initialize logging");
    debug!("Run with {:?}", arguments);

    let broker = Broker::new(arguments.script_dir, arguments.timeout);
    debug!("Start event broker with {:?}", broker);

    if arguments.run_startup_triggers {
        info!("Found '--run-startup-triggers'. Start execute all scripts for the current state for each interface");
        if let Err(err) = broker.trigger_all().await {
            warn!("{}", err);
        }
    }

    broker.listen().await
}

fn main() {
    process::exit(match task::block_on(run_app()) {
        Ok(_) => 0,
        Err(err) => {
            error!("{}", err);
            1
        }
    });
}
