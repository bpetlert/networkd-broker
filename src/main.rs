use anyhow::{bail, Context, Result};
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
    if let Err(err) = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .without_time()
        .with_ansi(false)
        .try_init()
    {
        bail!("Failed to initialize tracing subscriber: {err}");
    }

    Ok(())
}

async fn run_app() -> anyhow::Result<()> {
    let arguments = Arguments::parse();
    init_log().context("Failed to initialize logging")?;
    debug!("Run with {:?}", arguments);

    let mut broker = Broker::new(arguments.script_dir, arguments.timeout).await?;

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
