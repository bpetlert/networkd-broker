use std::{io, process::ExitCode};

use anyhow::{anyhow, Context, Result};
use async_std::task;
use clap::Parser;
use mimalloc::MiMalloc;
use tracing::{debug, error, info, warn};
use tracing_subscriber::EnvFilter;

use networkd_broker::{args::Arguments, broker::Broker};

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

fn run() -> Result<()> {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or(EnvFilter::try_new("networkd_broker=info")?);
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .without_time()
        .with_writer(io::stderr)
        .try_init()
        .map_err(|err| anyhow!("{err:#}"))
        .context("Failed to initialize tracing subscriber")?;

    let arguments = Arguments::parse();
    debug!("Run with {:?}", arguments);

    task::block_on(async {
        let mut broker = Broker::new(arguments.script_dir, arguments.timeout)
            .await
            .context("Failed to create broker thread")?;

        if arguments.run_startup_triggers {
            info!("Found '--run-startup-triggers'. Start execute all scripts for the current state for each interface");
            if let Err(err) = broker
                .trigger_all()
                .await
                .context("Failed to run startup-triggers")
            {
                warn!("{err:#}");
            }
        }

        broker
            .listen()
            .await
            .context("Could not start broker thread")
    })
}

fn main() -> ExitCode {
    if let Err(err) = run() {
        error!("{err:#}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
