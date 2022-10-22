use crate::dbus_interface::NetworkManagerProxy;
use anyhow::{anyhow, bail, Result};
use serde::Deserialize;
use tracing::debug;
use zbus::{Message, MessageType};

#[derive(Deserialize)]
pub struct LinkDetails {
    #[serde(rename = "AdministrativeState")]
    administrative_state: String,

    #[serde(rename = "OperationalState")]
    pub operational_state: String,

    #[serde(rename = "CarrierState")]
    carrier_state: String,

    #[serde(rename = "AddressState")]
    address_state: String,

    #[serde(rename = "IPv4AddressState")]
    ipv4_address_state: String,

    #[serde(rename = "IPv6AddressState")]
    ipv6_address_state: String,
}

#[derive(Debug)]
struct Link {
    index: i32,
    name: String,
    _path: String,
}

/// Network link information which is extracted from DBus signal message
pub struct LinkEvent {
    pub iface: String,
    pub state: String,
    pub path: String,
    pub link_details: LinkDetails,
    pub link_details_json: String,
}

impl LinkEvent {
    /// Extract link event from DBus signal message
    pub async fn new(msg: &Message, conn: &::zbus::Connection) -> Result<Box<LinkEvent>> {
        if msg.message_type() != MessageType::Signal {
            bail!("Event message {:?} is not dbus signal", msg.message_type());
        }

        if &*msg.interface().unwrap() != "org.freedesktop.DBus.Properties" {
            bail!(
                "{} is not 'org.freedesktop.DBus.Properties'",
                &*msg.interface().unwrap()
            );
        }

        let path: String = if let Some(path) = msg.path() {
            path.as_str().to_string()
        } else {
            bail!("Invalid path: {:?}", &msg);
        };

        let link = LinkEvent::link_from_path(&path, conn).await?;
        debug!("Get link details of {link:?}");
        let proxy = NetworkManagerProxy::new(conn).await?;
        let describe_link = proxy.describe_link(link.index).await?;
        let link_details = match serde_json::from_str::<LinkDetails>(&describe_link) {
            Ok(link_details) => {
                debug!(
                    "AdministrativeState: {a}\
                    ,  OperationalState: {b}\
                    ,  CarrierState: {c}\
                    ,  AddressState: {d}\
                    ,  IPv4AddressState: {e}\
                    ,  IPv6AddressState: {f}",
                    a = link_details.administrative_state,
                    b = link_details.operational_state,
                    c = link_details.carrier_state,
                    d = link_details.address_state,
                    e = link_details.ipv4_address_state,
                    f = link_details.ipv6_address_state
                );
                link_details
            }
            Err(err) => bail!("Cannot get link state: {err}"),
        };

        Ok(Box::new(LinkEvent {
            iface: link.name,
            state: link_details.operational_state.clone(),
            path: msg.to_string(),
            link_details,
            link_details_json: describe_link,
        }))
    }

    async fn link_from_path(path: &str, conn: &::zbus::Connection) -> Result<Link> {
        let proxy = NetworkManagerProxy::new(conn).await?;
        let links = proxy.list_links().await?;
        for (index, name, p) in links {
            if path == p.as_str() {
                return Ok(Link {
                    index,
                    name,
                    _path: p.to_string(),
                });
            }
        }
        Err(anyhow!("No iface found on {path}"))
    }
}

impl std::fmt::Display for LinkEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{} --> {}", self.iface, self.state)
    }
}
