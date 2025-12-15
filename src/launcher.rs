use std::{
    sync::mpsc::{
        RecvError,
        Sender,
        channel,
    },
    thread,
};

use anyhow::{
    Context,
    Result,
};
use tracing::{
    debug,
    error,
    warn,
};

use crate::script::Script;

#[derive(Debug)]
pub struct Launcher {
    tx: Sender<Box<Script>>,
}

impl Launcher {
    pub fn new() -> Result<Self> {
        let (tx, rx) = channel::<Box<Script>>();

        thread::Builder::new()
            .name("script launcher".to_string())
            .spawn(move || {
                loop {
                    match rx.recv() {
                        Ok(script) => {
                            debug!("Received a script {script:?}");
                            if let Err(err) = script.execute().context("Failed to execute script") {
                                warn!("{err:#}");
                            }
                        }
                        Err(RecvError {}) => {
                            error!("Failed to receive script");
                        }
                    };
                }
            })
            .context("Could not create script launcher thread")?;

        Ok(Launcher { tx })
    }

    pub fn add(&self, script: Script) -> Result<()> {
        self.tx
            .send(Box::new(script))
            .context("Failed to send a script to launcher channel")?;
        Ok(())
    }
}
