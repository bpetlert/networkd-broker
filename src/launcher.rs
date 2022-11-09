use crate::script::Script;
use anyhow::Result;
use std::{
    sync::mpsc::{channel, RecvError, Sender},
    thread,
};
use tracing::warn;

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
                    if let Err(err) = script.execute() {
                        warn!("{err}");
                    }
                }
                Err(RecvError {}) => {}
            };
        });

        Launcher { tx }
    }

    pub fn add(&self, script: Script) -> Result<()> {
        self.tx.send(Box::new(script))?;
        Ok(())
    }
}

impl Default for Launcher {
    fn default() -> Self {
        Self::new()
    }
}
