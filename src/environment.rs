use std::collections::HashMap;

use strum_macros::{Display, EnumString};

#[derive(Debug, PartialEq, EnumString, Display)]
pub enum ScriptEnvironment {
    #[strum(serialize = "NWD_DEVICE_IFACE")]
    DeviceIface,

    #[strum(serialize = "NWD_DISPATCHER_ACTION")]
    DispatcherAction,

    #[strum(serialize = "NWD_ESSID")]
    Essid,

    /// Access point's MAC address
    #[strum(serialize = "NWD_STATION")]
    Station,

    #[strum(serialize = "NWD_IP4_ADDRESS")]
    Ip4Address,

    #[strum(serialize = "NWD_IP6_ADDRESS")]
    Ip6Address,

    #[strum(serialize = "NWD_ADMINISTRATIVE_STATE")]
    AdministrativeState,

    #[strum(serialize = "NWD_OPERATIONAL_STATE")]
    OperationalState,

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

    pub fn add<V>(&mut self, name: ScriptEnvironment, value: V) -> &mut Environments
    where
        V: Into<String>,
    {
        self.envs.insert(name.to_string(), value.into());
        self
    }
}
