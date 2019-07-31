use std::{collections::HashMap, str::FromStr};

use strum_macros::{Display, EnumString};

use dbus::{
    arg::RefArg,
    stdintf::org_freedesktop_dbus::PropertiesPropertiesChanged as PC,
    {Message, MessageType, SignalArgs},
};

use serde_json::{Map, Value};

use crate::{
    error::{AppError, Result},
    extcommand::ExtCommand,
};

#[derive(Debug, PartialEq, EnumString, Display)]
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

/// Operational status
///
/// Taken from networkctl's man page
#[derive(Debug, PartialEq, EnumString, Display)]
pub enum OperationalStatus {
    #[strum(serialize = "n/a")]
    NotAvailable,

    #[strum(serialize = "off")]
    Off,

    #[strum(serialize = "no-carrier")]
    NoCarrier,

    #[strum(serialize = "dormant")]
    Dormant,

    #[strum(serialize = "degraded-carrier")]
    DegradedCarrier,

    #[strum(serialize = "carrier")]
    Carrier,

    #[strum(serialize = "degraded")]
    Degraded,

    #[strum(serialize = "enslaved")]
    Enslaved,

    #[strum(serialize = "routable")]
    Routable,

    #[strum(serialize = "pending")]
    Pending,

    #[strum(serialize = "failed")]
    Failed,

    #[strum(serialize = "configuring")]
    Configuring,

    #[strum(serialize = "configured")]
    Configured,

    #[strum(serialize = "unmanaged")]
    Unmanaged,

    #[strum(serialize = "linger")]
    Linger,
}

/// Network link information which is extracted from DBus signal message
#[derive(Debug)]
pub struct LinkEvent<'m> {
    pub path: dbus::Path<'m>,
    pub state_type: StateType,
    pub state: OperationalStatus,
}

impl LinkEvent<'_> {
    /// Extract link event from DBus signal message
    pub fn from_message(msg: &Message) -> Result<Box<LinkEvent>> {
        if msg.msg_type() != MessageType::Signal {
            return Err(AppError::NotSignal);
        }

        if &*msg.interface().unwrap() != "org.freedesktop.DBus.Properties" {
            return Err(AppError::NotDBusProperties);
        }

        if let Some(pc) = PC::from_message(&msg) {
            if pc.interface_name != "org.freedesktop.network1.Link" {
                return Err(AppError::NotLinkEvent);
            }

            let (state_type, state) = pc.changed_properties.iter().next().unwrap();

            let st = match StateType::from_str(state_type.as_ref()) {
                Ok(st) => st,
                Err(_) => return Err(AppError::InvalidStateType),
            };

            let s = match OperationalStatus::from_str(state.as_str().unwrap()) {
                Ok(s) => s,
                Err(_) => return Err(AppError::InvalidOperationalStatus),
            };

            return Ok(Box::new(LinkEvent {
                path: msg.path().unwrap(),
                state_type: st,
                state: s,
            }));
        }

        Err(AppError::CannotConvertEventMessage)
    }

    /// Convert DBus path to network interface index
    ///
    /// The first character of each component of a dbus object path is escaped, if it is a number.
    ///
    ///     1 → _31
    ///     2 → _32
    ///     3 → _33
    ///    10 → _310
    ///
    /// _31 --> 0x31 --> '1'
    ///
    /// src: https://lists.freedesktop.org/archives/systemd-devel/2016-May/036528.html
    pub fn index(&self) -> Result<u8> {
        let components = self.path.split('/').collect::<Vec<&str>>();
        if components.len() != 6 {
            return Err(AppError::LinkToIndex);
        }

        let escaped_index = components.last().unwrap();
        if escaped_index.len() < 3 {
            return Err(AppError::LinkToIndex);
        }

        let first_char: char = match u8::from_str_radix(&escaped_index[1..3], 16) {
            Ok(c) => c as char,
            Err(_) => return Err(AppError::LinkToIndex),
        };

        let the_rest = &escaped_index[3..];
        let index: String = first_char.to_string() + the_rest;
        match index.parse::<u8>() {
            Ok(i) => Ok(i),
            Err(_) => Err(AppError::LinkToIndex),
        }
    }
}

#[derive(Debug, PartialEq, EnumString, Display)]
pub enum LinkType {
    #[strum(serialize = "loopback")]
    Loopback,

    #[strum(serialize = "ether")]
    Ether,

    #[strum(serialize = "wlan")]
    Wlan,
}

#[derive(Debug, PartialEq)]
pub struct Link {
    pub idx: u8,
    pub iface: String,
    pub link_type: LinkType,
    pub operational: OperationalStatus,
    pub setup: OperationalStatus,
}

impl Link {
    pub fn new() -> Link {
        Link {
            idx: 0,
            iface: String::new(),
            link_type: LinkType::Loopback,
            operational: OperationalStatus::Linger,
            setup: OperationalStatus::Unmanaged,
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

    pub fn link_type<S>(&mut self, link_type: S) -> &mut Link
    where
        S: AsRef<str>,
    {
        self.link_type = LinkType::from_str(link_type.as_ref()).unwrap();
        self
    }

    pub fn operational(&mut self, s: OperationalStatus) -> &mut Link {
        self.operational = s;
        self
    }

    pub fn setup(&mut self, s: OperationalStatus) -> &mut Link {
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
        let link = LinkEvent::from_message(&msg);
        assert_eq!(link.unwrap_err(), AppError::NotSignal);

        // Invalid interface 'org.freedesktop.DBus.Properties'
        let msg = Message::new_signal(
            "/org/freedesktop/network1/link/_33",
            "org.freedesktop.DBus",
            "PropertiesChanged",
        )
        .unwrap();
        let link = LinkEvent::from_message(&msg);
        assert_eq!(link.unwrap_err(), AppError::NotDBusProperties);

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
        let link = LinkEvent::from_message(&msg);
        assert_eq!(link.unwrap_err(), AppError::CannotConvertEventMessage);
    }

    #[test]
    #[ignore]
    fn test_link_event_from_message_with_valid_msg() {
        // TODO: Test link event with valid message
        unimplemented!();
    }

    #[test]
    fn test_dbus_path_to_network_link_index() {
        let mut link_event = LinkEvent {
            path: dbus::Path::new("/org/freedesktop/network1/link").unwrap(),
            state_type: StateType::OperationalState,
            state: OperationalStatus::Off,
        };
        assert_eq!(link_event.index().unwrap_err(), AppError::LinkToIndex);

        link_event.path = dbus::Path::new("/org/freedesktop/network1/link/_").unwrap();
        assert_eq!(link_event.index().unwrap_err(), AppError::LinkToIndex);

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
}
