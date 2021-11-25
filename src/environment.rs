use crate::link::{Link, LinkEvent};
use lazy_static::lazy_static;
use regex::Regex;
use serde_json::{Map, Value};
use std::collections::HashMap;
use strum_macros::{Display, EnumString};

#[derive(Debug, PartialEq, EnumString, Display)]
pub enum ScriptEnvironment {
    #[strum(serialize = "NWD_DEVICE_IFACE")]
    DeviceIface,

    #[strum(serialize = "NWD_BROKER_ACTION")]
    BrokerAction,

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

    pub fn extract_from(
        &mut self,
        link_event: &LinkEvent,
        link: &Link,
        status: Map<String, Value>,
        json: bool,
    ) -> &mut Environments {
        lazy_static! {
            static ref IPV4_PATTERN: Regex = Regex::new(include_str!("ipv4.regex"))
                .expect("Cannot create regex for IPv4 pattern.");
        }

        // Extract IPV4
        let mut ip4_address = String::new();
        if let Some(value) = status.get("Address") {
            match value {
                Value::String(s) => {
                    if IPV4_PATTERN.is_match(s) {
                        ip4_address = s.to_string();
                    }
                }
                Value::Array(a) => {
                    let addr: Vec<&str> = a
                        .iter()
                        .filter(|b| IPV4_PATTERN.is_match(b.as_str().unwrap()))
                        .map(|s| s.as_str().unwrap_or_default())
                        .collect();
                    ip4_address = addr.join(" ");
                }
                _ => {}
            }
        }

        // Extract IPV6
        let mut ip6_address = String::new();
        if let Some(value) = status.get("Address") {
            match value {
                Value::String(s) => {
                    if !IPV4_PATTERN.is_match(s) {
                        ip6_address = s.to_string();
                    }
                }
                Value::Array(a) => {
                    let addr: Vec<&str> = a
                        .iter()
                        .filter(|b| !IPV4_PATTERN.is_match(b.as_str().unwrap()))
                        .map(|s| s.as_str().unwrap_or_default())
                        .collect();
                    ip6_address = addr.join(" ");
                }
                _ => {}
            }
        }

        self
            // NWD_DEVICE_IFACE
            .add(ScriptEnvironment::DeviceIface, &link.iface)
            // NWD_BROKER_ACTION
            .add(
                ScriptEnvironment::BrokerAction,
                link_event.state.to_string(),
            )
            // NWD_ESSID
            .add(
                ScriptEnvironment::Essid,
                status
                    .get("Ssid")
                    .map_or("", |s| s.as_str().unwrap_or_default()),
            )
            // NWD_STATION
            .add(
                ScriptEnvironment::Station,
                status
                    .get("Station")
                    .map_or("", |s| s.as_str().unwrap_or_default()),
            )
            // NWD_IP4_ADDRESS
            .add(ScriptEnvironment::Ip4Address, ip4_address)
            // NWD_IP6_ADDRESS
            .add(ScriptEnvironment::Ip6Address, ip6_address)
            // NWD_ADMINISTRATIVE_STATE
            .add(
                ScriptEnvironment::AdministrativeState,
                link.setup.to_string(),
            )
            // NWD_OPERATIONAL_STATE
            .add(
                ScriptEnvironment::OperationalState,
                link.operational.to_string(),
            );

        // NWD_JSON
        if json {
            self.add(
                ScriptEnvironment::Json,
                serde_json::to_string(&status).unwrap_or_default(),
            );
        }

        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::extcommand::ExtCommand;
    use crate::link::{LinkEvent, LinkStatus, LinkType, StateType};

    #[test]
    fn test_extract_from() {
        let link_event = LinkEvent {
            path: dbus::Path::new("/org/freedesktop/network1/link/_32")
                .expect("Cannot create DBus path."),
            state_type: StateType::AdministrativeState,
            state: LinkStatus::Configured,
        };

        let mut link2 = Link::new();
        link2
            .idx(2)
            .iface("wlan0")
            .link_type(LinkType::Wlan)
            .operational(LinkStatus::Routable)
            .setup(LinkStatus::Configured);

        let networkctl_status2 = include_str!("networkctl_status_test_2.raw");
        let mut status2 =
            ExtCommand::parse_networkctl_status(networkctl_status2.as_bytes().to_vec())
                .expect("Cannot parse networkctl status.");
        status2.insert("Ssid".to_owned(), Value::String("Haven".to_owned()));
        status2.insert(
            "Station".to_owned(),
            Value::String("19:21:12:bf:23:c6".to_owned()),
        );

        let mut envs = Environments::new();
        envs.extract_from(&link_event, &link2, status2, false);

        let pack = envs.pack();
        assert_eq!(pack.len(), 8);

        assert_eq!(
            pack.get("NWD_IP4_ADDRESS"),
            Some(&"192.168.1.106 (DHCP4 via 192.168.1.254)".to_owned())
        );
        assert_eq!(pack.get("NWD_DEVICE_IFACE"), Some(&"wlan0".to_owned()));
        assert_eq!(
            pack.get("NWD_IP6_ADDRESS"),
            Some(&"fb62:bfa:cbf7:0:7be3:1dcf:fddb:9e25 fd70::7be3:1dcf:fddb:9e25".to_owned())
        );
        assert_eq!(
            pack.get("NWD_STATION"),
            Some(&"19:21:12:bf:23:c6".to_owned())
        );
        assert_eq!(
            pack.get("NWD_OPERATIONAL_STATE"),
            Some(&"routable".to_owned())
        );
        assert_eq!(
            pack.get("NWD_ADMINISTRATIVE_STATE"),
            Some(&"configured".to_owned())
        );
        assert_eq!(pack.get("NWD_ESSID"), Some(&"Haven".to_owned()));
        assert_eq!(
            pack.get("NWD_BROKER_ACTION"),
            Some(&"configured".to_owned())
        );
    }
}
