use crate::{args::Arguments, broker::Broker};
use anyhow::{bail, Result};
use async_std::task;
use clap::Parser;
use mimalloc::MiMalloc;
use std::io;
use tracing::{debug, info, warn};
use tracing_subscriber::EnvFilter;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

mod args;
mod broker;
mod dbus_interface;
mod launcher;
mod link;
mod script;

fn main() -> Result<()> {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or(EnvFilter::try_new("networkd_broker=info")?);
    if let Err(err) = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .without_time()
        .with_writer(io::stderr)
        .try_init()
    {
        bail!("Failed to initialize tracing subscriber: {err}");
    }

    let arguments = Arguments::parse();
    debug!("Run with {:?}", arguments);

    task::block_on(async {
        let mut broker = Broker::new(arguments.script_dir, arguments.timeout).await?;

        if arguments.run_startup_triggers {
            info!("Found '--run-startup-triggers'. Start execute all scripts for the current state for each interface");
            if let Err(err) = broker.trigger_all().await {
                warn!("{}", err);
            }
        }

        broker.listen().await
    })
}
