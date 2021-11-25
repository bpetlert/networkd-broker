use anyhow::Result;
use std::{
    sync::mpsc::{channel, RecvError, Sender},
    thread,
};

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
                    let _ = script.execute();
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
