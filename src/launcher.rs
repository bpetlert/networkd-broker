use std::{
    sync::mpsc::{channel, RecvError, Sender},
    thread,
};

use anyhow::{Context, Result};
use tracing::{debug, error, warn};

use crate::script::Script;

#[derive(Debug)]
pub struct Launcher {
    tx: Sender<Box<Script>>,
}

impl Launcher {
    pub fn new() -> Launcher {
        let (tx, rx) = channel::<Box<Script>>();

        thread::spawn(move || loop {
            match rx.recv() {
                Ok(script) => {
                    debug!("Received a script {script:?}");
                    if let Err(err) = script.execute() {
                        warn!("{err}");
                    }
                }
                Err(RecvError {}) => {
                    error!("Failed to receive script");
                }
            };
        });

        Launcher { tx }
    }

    pub fn add(&self, script: Script) -> Result<()> {
        self.tx
            .send(Box::new(script))
            .context("Failed to send a script to launcher channel")?;
        Ok(())
    }
}

impl Default for Launcher {
    fn default() -> Self {
        Self::new()
    }
}
