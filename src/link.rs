use crate::extcommand::ExtCommand;
use anyhow::{anyhow, Result};
use dbus::arg::RefArg;
use dbus::blocking::stdintf::org_freedesktop_dbus::PropertiesPropertiesChanged as Ppc;
use dbus::message::{Message, MessageType, SignalArgs};
use log::debug;
use serde_json::{Map, Value};
use std::{collections::HashMap, str::FromStr};
use strum_macros::{Display, EnumString};

#[derive(Debug, Clone, PartialEq, EnumString, Display)]
pub enum StateType {
    /// 'AdministrativeState' field of DBus signal message
    /// or 'SETUP' field of 'networkctl list'
    #[strum(serialize = "AdministrativeState")]
    AdministrativeState,

    /// 'OperationalState' field of DBus signal message
    /// or 'OPERATIONAL' of 'networkctl list'
    #[strum(serialize = "OperationalState")]
    OperationalState,
}

/// Link status
///
/// Operational Status / Setup Status from man page of networkctl (systemd 247.2)
#[derive(Debug, Clone, PartialEq, EnumString, Display)]
pub enum LinkStatus {
    /// the device is missing
    #[strum(serialize = "missing")]
    Missing,

    /// the device is powered down
    #[strum(serialize = "off")]
    Off,

    /// the device is powered up, but it does not yet have a carrier
    #[strum(serialize = "no-carrier")]
    NoCarrier,

    /// the device has a carrier, but is not yet ready for normal traffic
    #[strum(serialize = "dormant")]
    Dormant,

    /// for bond or bridge master, one of the bonding or bridge slave network interfaces is in off, no-carrier, or dormant state
    #[strum(serialize = "degraded-carrier")]
    DegradedCarrier,

    /// the link has a carrier, or for bond or bridge master, all bonding or bridge slave network interfaces are enslaved to the master.
    #[strum(serialize = "carrier")]
    Carrier,

    /// the link has carrier and addresses valid on the local link configured
    #[strum(serialize = "degraded")]
    Degraded,

    /// the link has carrier and is enslaved to bond or bridge master network interface
    #[strum(serialize = "enslaved")]
    Enslaved,

    /// the link has carrier and routable address configured
    #[strum(serialize = "routable")]
    Routable,

    /// udev is still processing the link, we don't yet know if we will manage it
    #[strum(serialize = "pending")]
    Pending,

    /// networkd failed to manage the link
    #[strum(serialize = "failed")]
    Failed,

    /// in the process of retrieving configuration or configuring the link
    #[strum(serialize = "configuring")]
    Configuring,

    /// link configured successfully
    #[strum(serialize = "configured")]
    Configured,

    /// networkd is not handling the link
    #[strum(serialize = "unmanaged")]
    Unmanaged,

    /// the link is gone, but has not yet been dropped by networkd
    #[strum(serialize = "linger")]
    Linger,
}

/// Network link information which is extracted from DBus signal message
#[derive(Debug)]
pub struct LinkEvent<'m> {
    pub path: dbus::Path<'m>,
    pub state_type: StateType,
    pub state: LinkStatus,
}

impl LinkEvent<'_> {
    /// Extract link event from DBus signal message
    pub fn from_message(msg: &Message) -> Result<Box<LinkEvent>> {
        if msg.msg_type() != MessageType::Signal {
            return Err(anyhow!(
                "Event message `{:?}` is not dbus signal",
                msg.msg_type()
            ));
        }

        if &*msg.interface().unwrap() != "org.freedesktop.DBus.Properties" {
            return Err(anyhow!(
                "`{}` is not 'org.freedesktop.DBus.Properties'",
                &*msg.interface().unwrap()
            ));
        }

        if let Some(pc) = Ppc::from_message(&msg) {
            debug!("Properties Changed: {:?}", &pc);
            if pc.interface_name != "org.freedesktop.network1.Link" {
                return Err(anyhow!(
                    "`{}` is not 'org.freedesktop.network1.Link'",
                    pc.interface_name
                ));
            }

            let (state_type, state) = pc.changed_properties.iter().next().unwrap();

            let st = match StateType::from_str(state_type.as_ref()) {
                Ok(st) => st,
                Err(e) => return Err(anyhow!("`{}` is invalid state type: {}", state_type, e)),
            };

            let s = match LinkStatus::from_str(state.as_str().unwrap()) {
                Ok(s) => s,
                Err(e) => {
                    return Err(anyhow!(
                        "`{}` is invalid operational status: {}",
                        state.as_str().unwrap(),
                        e
                    ))
                }
            };

            return Ok(Box::new(LinkEvent {
                path: msg.path().unwrap(),
                state_type: st,
                state: s,
            }));
        }

        Err(anyhow!("Cannot convert event message: {:?}", &msg))
    }

    /// Convert DBus path to network interface index
    ///
    /// The first character of each component of a dbus object path is escaped, if it is a number.
    ///
    /// ```ignore
    ///     1 → _31
    ///     2 → _32
    ///     3 → _33
    ///    10 → _310
    ///
    /// _31 --> 0x31 --> '1'
    /// ```
    ///
    /// src: https://lists.freedesktop.org/archives/systemd-devel/2016-May/036528.html
    pub fn index(&self) -> Result<u8> {
        let components = self.path.split('/').collect::<Vec<&str>>();
        if components.len() != 6 {
            return Err(anyhow!("Cannot convert link `{}` to index", &self.path));
        }

        let escaped_index = components.last().unwrap();
        if escaped_index.len() < 3 {
            return Err(anyhow!("Cannot convert link `{}` to index.", &self.path));
        }

        let first_char: char = match u8::from_str_radix(&escaped_index[1..3], 16) {
            Ok(c) => c as char,
            Err(e) => {
                return Err(anyhow!(
                    "Cannot convert link `{}` to index: {}",
                    &self.path,
                    e
                ))
            }
        };

        let the_rest = &escaped_index[3..];
        let index: String = first_char.to_string() + the_rest;
        match index.parse::<u8>() {
            Ok(i) => Ok(i),
            Err(e) => Err(anyhow!(
                "Cannot convert link `{}` to index: {}",
                &self.path,
                e
            )),
        }
    }

    pub fn index_to_dbus_path(idx: u8) -> String {
        let idx_str: String = idx.to_string();
        let chars: Vec<&str> = idx_str.split("").collect();
        let first_part: u8 = chars[1].as_bytes()[0];
        format!(
            "/org/freedesktop/network1/link/_{:x}{}",
            first_part,
            chars[2..].join("")
        )
    }
}

#[derive(Debug, Clone, PartialEq, EnumString, Display)]
pub enum LinkType {
    #[strum(serialize = "loopback")]
    Loopback,

    #[strum(serialize = "ether")]
    Ether,

    #[strum(serialize = "wlan")]
    Wlan,

    #[strum(serialize = "wwan")]
    Wwan,

    #[strum(serialize = "wimax")]
    Wimax,

    #[strum(serialize = "bridge")]
    Bridge,

    #[strum(default)]
    Unknown(String),
}

#[derive(Debug, PartialEq)]
pub struct Link {
    pub idx: u8,
    pub iface: String,
    pub link_type: LinkType,
    pub operational: LinkStatus,
    pub setup: LinkStatus,
}

impl Link {
    pub fn new() -> Link {
        Link {
            idx: 0,
            iface: String::new(),
            link_type: LinkType::Loopback,
            operational: LinkStatus::Linger,
            setup: LinkStatus::Unmanaged,
        }
    }

    pub fn idx(&mut self, idx: u8) -> &mut Link {
        self.idx = idx;
        self
    }

    pub fn iface<S>(&mut self, iface: S) -> &mut Link
    where
        S: Into<String>,
    {
        self.iface = iface.into();
        self
    }

    pub fn link_type(&mut self, link_type: LinkType) -> &mut Link {
        self.link_type = link_type;
        self
    }

    pub fn operational(&mut self, s: LinkStatus) -> &mut Link {
        self.operational = s;
        self
    }

    pub fn setup(&mut self, s: LinkStatus) -> &mut Link {
        self.setup = s;
        self
    }

    pub fn status(&self) -> Result<Map<String, Value>> {
        ExtCommand::link_status(&self)
    }

    pub fn link_list() -> Result<HashMap<u8, Link>> {
        ExtCommand::link_list()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_link_event_from_message_with_invalid_msg() {
        // Non signal message
        let msg = Message::new_method_call("org.test.rust", "/", "org.test.rust", "Test").unwrap();
        let result = LinkEvent::from_message(&msg);
        assert!(result.is_err());

        // Invalid interface 'org.freedesktop.DBus.Properties'
        let msg = Message::new_signal(
            "/org/freedesktop/network1/link/_33",
            "org.freedesktop.DBus",
            "PropertiesChanged",
        )
        .unwrap();
        let result = LinkEvent::from_message(&msg);
        assert!(result.is_err());

        // TODO: Test invalid "org.freedesktop.network1.Link"

        // TODO: Test "invalid state type"

        // TODO: Test "invalid operational status"

        // Invalid message
        let msg = Message::new_signal(
            "/org/freedesktop/network1/link/_33",
            "org.freedesktop.DBus.Properties",
            "PropertiesChanged",
        )
        .unwrap();
        let result = LinkEvent::from_message(&msg);
        assert!(result.is_err());
    }

    #[test]
    #[ignore]
    fn test_link_event_from_message_with_valid_msg() {
        // TODO: Test link event with valid message
        todo!()
    }

    #[test]
    fn test_dbus_path_to_network_link_index() {
        let mut link_event = LinkEvent {
            path: dbus::Path::new("/org/freedesktop/network1/link").unwrap(),
            state_type: StateType::OperationalState,
            state: LinkStatus::Off,
        };
        assert!(link_event.index().is_err());

        link_event.path = dbus::Path::new("/org/freedesktop/network1/link/_").unwrap();
        assert!(link_event.index().is_err());

        link_event.path = dbus::Path::new("/org/freedesktop/network1/link/_31").unwrap();
        assert_eq!(link_event.index().unwrap(), 1);

        link_event.path = dbus::Path::new("/org/freedesktop/network1/link/_32").unwrap();
        assert_eq!(link_event.index().unwrap(), 2);

        link_event.path = dbus::Path::new("/org/freedesktop/network1/link/_33").unwrap();
        assert_eq!(link_event.index().unwrap(), 3);

        link_event.path = dbus::Path::new("/org/freedesktop/network1/link/_34").unwrap();
        assert_eq!(link_event.index().unwrap(), 4);

        link_event.path = dbus::Path::new("/org/freedesktop/network1/link/_310").unwrap();
        assert_eq!(link_event.index().unwrap(), 10);
    }

    #[test]
    fn test_to_dbus_path() {
        let dbus_path = LinkEvent::index_to_dbus_path(1);
        assert_eq!(dbus_path, "/org/freedesktop/network1/link/_31");

        let dbus_path = LinkEvent::index_to_dbus_path(2);
        assert_eq!(dbus_path, "/org/freedesktop/network1/link/_32");

        let dbus_path = LinkEvent::index_to_dbus_path(3);
        assert_eq!(dbus_path, "/org/freedesktop/network1/link/_33");

        let dbus_path = LinkEvent::index_to_dbus_path(10);
        assert_eq!(dbus_path, "/org/freedesktop/network1/link/_310");
    }
}
