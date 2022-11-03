use crate::link::LinkEvent;
use anyhow::Result;
use std::collections::HashMap;
use strum::{Display, EnumString};

#[derive(Debug, PartialEq, Eq, EnumString, Display)]
pub enum ScriptEnvironment {
    #[strum(serialize = "NWD_DEVICE_IFACE")]
    DeviceIface,

    #[strum(serialize = "NWD_BROKER_ACTION")]
    BrokerAction,

    #[strum(serialize = "NWD_JSON")]
    Json,
}

#[derive(Debug)]
pub struct Environments {
    envs: HashMap<String, String>,
}

impl Environments {
    pub fn new() -> Environments {
        Environments {
            envs: HashMap::new(),
        }
    }

    pub fn pack(&self) -> &HashMap<String, String> {
        &self.envs
    }

    pub fn add(&mut self, name: ScriptEnvironment, value: String) -> &mut Environments {
        self.envs.insert(name.to_string(), value);
        self
    }

    pub fn pack_from(&mut self, event: &LinkEvent) -> Result<()> {
        self.add(ScriptEnvironment::DeviceIface, event.iface.clone())
            .add(ScriptEnvironment::BrokerAction, event.state.clone())
            .add(ScriptEnvironment::Json, event.link_details_json.clone());

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_script_env() {
        let mut envs = Environments::new();
        envs.add(ScriptEnvironment::DeviceIface, "wlp3s0".to_string())
            .add(ScriptEnvironment::BrokerAction, "routable".to_string());

        let pack = envs.pack();
        assert_eq!(pack.len(), 2);
        assert_eq!(pack.get("NWD_DEVICE_IFACE"), Some(&"wlp3s0".to_string()));
        assert_eq!(pack.get("NWD_BROKER_ACTION"), Some(&"routable".to_string()));
    }
}
