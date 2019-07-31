use std::{collections::HashMap, process::Command, str::FromStr};

use lazy_static::lazy_static;
use regex::{Regex, RegexSet, SetMatches};

use serde_json::{Map, Number, Value};

use crate::{
    error::{AppError, Result},
    link::{Link, LinkType, OperationalStatus},
};

#[derive(Debug)]
pub struct ExtCommand;

impl ExtCommand {
    pub fn link_list() -> Result<HashMap<u8, Link>> {
        ExtCommand::call_networkctl_list()
    }

    pub fn link_status(link: &Link) -> Result<Map<String, Value>> {
        let mut info = ExtCommand::call_networkctl_status(&link.iface)?;

        if link.link_type == LinkType::Wlan {
            let iw_info = ExtCommand::call_iw_link(&link.iface)?;
            for (key, val) in iw_info {
                info.insert(key, val);
            }
        }

        Ok(info)
    }

    fn call_networkctl_list() -> Result<HashMap<u8, Link>> {
        // Call 'networkctl list --no-pager --no-legend'
        let output = match Command::new("networkctl")
            .args(&["list", "--no-pager", "--no-legend"])
            .output()
        {
            Ok(o) => o,
            Err(_) => return Err(AppError::CallNetworkctlFailed),
        };

        if !output.status.success() {
            return Err(AppError::CallNetworkctlFailed);
        }

        ExtCommand::parse_networkctl_list(output.stdout)
    }

    fn call_networkctl_status<S>(iface: S) -> Result<Map<String, Value>>
    where
        S: AsRef<str>,
    {
        // Call 'networkctl status --no-pager <iface>'
        let output = match Command::new("networkctl")
            .args(&["status", "--no-pager", iface.as_ref()])
            .output()
        {
            Ok(o) => o,
            Err(_) => {
                return Err(AppError::CallNetworkctlFailed);
            }
        };

        if !output.status.success() {
            return Err(AppError::CallNetworkctlFailed);
        }

        if output.stdout.is_empty() {
            return Err(AppError::LinkNotExist);
        }

        ExtCommand::parse_networkctl_status(output.stdout)
    }

    fn call_iw_link<S>(iface: S) -> Result<Map<String, Value>>
    where
        S: AsRef<str>,
    {
        let output = match Command::new("iw")
            .args(&["dev", iface.as_ref(), "link"])
            .output()
        {
            Ok(o) => o,
            Err(_) => {
                return Err(AppError::CallIwlFailed);
            }
        };

        if let Some(code) = output.status.code() {
            // command failed: No such device (-19)
            if code == 237 {
                return Err(AppError::LinkNotExist);
            }
        }

        if !output.status.success() {
            return Err(AppError::CallIwlFailed);
        }

        if output.stdout == b"Not connected.\x0A".to_vec() {
            return Err(AppError::NotConnected);
        }

        ExtCommand::parse_iw_link(output.stdout)
    }

    fn parse_networkctl_list(raw_output: Vec<u8>) -> Result<HashMap<u8, Link>> {
        lazy_static! {
            static ref PATTERN: Regex = Regex::new(include_str!("networkctl_list.regex")).unwrap();
        }

        let mut links: HashMap<u8, Link> = HashMap::new();
        String::from_utf8(raw_output)
            .unwrap()
            .lines()
            .filter_map(|line| PATTERN.captures(line))
            .map(|cap| {
                let idx = cap.name("idx").unwrap().as_str().parse::<u8>().unwrap();
                let iface = cap.name("iface").unwrap().as_str();
                let link_type = cap.name("type").unwrap().as_str();

                let operational =
                    OperationalStatus::from_str(cap.name("operational").unwrap().as_str()).unwrap();

                let setup =
                    OperationalStatus::from_str(cap.name("setup").unwrap().as_str()).unwrap();

                let mut ln = Link::new();
                ln.idx(idx)
                    .iface(iface)
                    .link_type(link_type)
                    .operational(operational)
                    .setup(setup);
                ln
            })
            .for_each(|x| {
                links.insert(x.idx, x);
            });

        Ok(links)
    }

    fn parse_networkctl_status(raw_output: Vec<u8>) -> Result<Map<String, Value>> {
        lazy_static! {
            static ref STATUS_PATTERN_SET: RegexSet = RegexSet::new(&[
                include_str!("networkctl_status_idx_link.regex"),
                include_str!("networkctl_status_field.regex"),
                include_str!("networkctl_status_extra_value.regex"),
            ])
            .unwrap();
            static ref LINK_PATTERN: Regex =
                Regex::new(include_str!("networkctl_status_idx_link.regex")).unwrap();
            static ref FIELD_PATTERN: Regex =
                Regex::new(include_str!("networkctl_status_field.regex")).unwrap();
            static ref EXTRA_VALUE_PATTERN: Regex =
                Regex::new(include_str!("networkctl_status_extra_value.regex")).unwrap();
        }

        let mut status: Map<String, Value> = Map::new();
        let mut last_insert_key: String = String::new();
        String::from_utf8(raw_output)
            .unwrap()
            .lines()
            .for_each(|line| {
                let matches: SetMatches = STATUS_PATTERN_SET.matches(line);
                if !matches.matched_any() {
                    return;
                }

                // Field
                if matches.matched(1) {
                    if let Some((key, value)) = FIELD_PATTERN.captures(line).and_then(|cap| {
                        cap.name("key")
                            .and_then(|key| cap.name("value").and_then(|value| Some((key, value))))
                    }) {
                        let key: String = key.as_str().to_owned().replace(char::is_whitespace, "");
                        last_insert_key = key.clone();
                        status.insert(key, Value::String(value.as_str().to_owned()));
                    }
                    return;
                }

                // Extra value for previous field
                if matches.matched(2) {
                    if let Some(extra_value) = EXTRA_VALUE_PATTERN
                        .captures(line)
                        .and_then(|cap| cap.name("extra_value"))
                    {
                        if let Some(last_insert_value) = status.get_mut(&last_insert_key) {
                            match last_insert_value {
                                Value::Array(v) => {
                                    v.push(Value::String(extra_value.as_str().to_owned()));
                                }
                                Value::String(s) => {
                                    *last_insert_value = Value::Array(vec![
                                        Value::String(s.clone()),
                                        Value::String(extra_value.as_str().to_owned()),
                                    ]);
                                }
                                _ => {}
                            };
                        }
                    }
                    return;
                }

                // Idx: Link
                if matches.matched(0) {
                    if let Some(cap) = LINK_PATTERN.captures(line) {
                        if let Some(idx) = cap.name("idx") {
                            if let Ok(n) = idx.as_str().parse::<u8>() {
                                status.insert("Idx".to_owned(), Value::Number(Number::from(n)));
                            }
                        }

                        if let Some(link) = cap.name("link") {
                            status
                                .insert("Link".to_owned(), Value::String(link.as_str().to_owned()));
                        }
                    }
                    return;
                }
            });

        Ok(status)
    }

    fn parse_iw_link(raw_output: Vec<u8>) -> Result<Map<String, Value>> {
        lazy_static! {
            static ref LINK_PATTERN: Regex = Regex::new(include_str!("iw_dev_link.regex")).unwrap();
        }

        let mut info: Map<String, Value> = Map::new();
        match LINK_PATTERN.captures(String::from_utf8(raw_output).unwrap().as_str()) {
            Some(c) => {
                if let Some(station) = c.name("station") {
                    info.insert(
                        "Station".to_owned(),
                        Value::String(station.as_str().to_owned()),
                    );
                }

                if let Some(ssid) = c.name("ssid") {
                    info.insert("Ssid".to_owned(), Value::String(ssid.as_str().to_owned()));
                }
            }
            None => {
                return Err(AppError::ParseIwLinkFailed);
            }
        }

        Ok(info)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_call_networkctl_list() {
        let link_list = ExtCommand::call_networkctl_list().unwrap();
        assert_ne!(link_list.len(), 0);
    }

    #[test]
    fn test_call_networkctl_status() {
        let info = ExtCommand::call_networkctl_status("lo").unwrap();
        assert_eq!(info.len(), 7);

        let err = ExtCommand::call_networkctl_status("LinkToOtherWorlds").unwrap_err();
        assert_eq!(err, AppError::LinkNotExist);
    }

    #[test]
    fn test_call_iw_link() {
        let err = ExtCommand::call_iw_link("lo").unwrap_err();
        assert_eq!(err, AppError::NotConnected);

        let err = ExtCommand::call_iw_link("LinkToOtherWorlds").unwrap_err();
        assert_eq!(err, AppError::LinkNotExist);
    }

    #[test]
    fn test_parse_networkctl_list() {
        let networkctl_list = include_str!("networkctl_list.test");
        let link_list =
            ExtCommand::parse_networkctl_list(networkctl_list.as_bytes().to_vec()).unwrap();
        assert_eq!(link_list.len(), 3);

        assert_eq!(
            link_list.get(&1),
            Some(&Link {
                idx: 1,
                iface: "lo".to_owned(),
                link_type: LinkType::Loopback,
                operational: OperationalStatus::Carrier,
                setup: OperationalStatus::Unmanaged,
            })
        );

        assert_eq!(
            link_list.get(&2),
            Some(&Link {
                idx: 2,
                iface: "wlp3s0".to_owned(),
                link_type: LinkType::Wlan,
                operational: OperationalStatus::Routable,
                setup: OperationalStatus::Configured,
            })
        );

        assert_eq!(
            link_list.get(&3),
            Some(&Link {
                idx: 3,
                iface: "enp6s0".to_owned(),
                link_type: LinkType::Ether,
                operational: OperationalStatus::Off,
                setup: OperationalStatus::Unmanaged,
            })
        );
    }

    #[test]
    fn test_parse_networkctl_status() {
        // Test link 1: lo
        let networkctl_status1 = include_str!("networkctl_status_1.test");
        let status1 =
            ExtCommand::parse_networkctl_status(networkctl_status1.as_bytes().to_vec()).unwrap();
        assert_eq!(status1.len(), 7);

        let networkctl_status1_json = include_str!("networkctl_status_1_json.test");
        let output1_json: Value = serde_json::from_str(networkctl_status1_json).unwrap();
        let status1_value: Value =
            serde_json::from_str(serde_json::to_string(&status1).unwrap().as_str()).unwrap();
        assert_eq!(&status1_value, &output1_json);

        // Test link 2: wlp3s0
        let networkctl_status2 = include_str!("networkctl_status_2.test");
        let status2 =
            ExtCommand::parse_networkctl_status(networkctl_status2.as_bytes().to_vec()).unwrap();
        assert_eq!(status2.len(), 15);

        let networkctl_status2_json = include_str!("networkctl_status_2_json.test");
        let output2_json: Value = serde_json::from_str(networkctl_status2_json).unwrap();
        let status2_value: Value =
            serde_json::from_str(serde_json::to_string(&status2).unwrap().as_str()).unwrap();
        assert_eq!(&status2_value, &output2_json);

        // Test link 3: enp6s0
        let networkctl_status3 = include_str!("networkctl_status_3.test");
        let status3 =
            ExtCommand::parse_networkctl_status(networkctl_status3.as_bytes().to_vec()).unwrap();
        assert_eq!(status3.len(), 11);

        let networkctl_status3_json = include_str!("networkctl_status_3_json.test");
        let output3_json: Value = serde_json::from_str(networkctl_status3_json).unwrap();
        let status3_value: Value =
            serde_json::from_str(serde_json::to_string(&status3).unwrap().as_str()).unwrap();
        assert_eq!(&status3_value, &output3_json);
    }

    #[test]
    fn test_parse_iw_link() {
        let iw_link = include_str!("iw_dev_link.test");
        let info = ExtCommand::parse_iw_link(iw_link.as_bytes().to_vec()).unwrap();
        assert_eq!(info.get("Station").unwrap(), "19:21:12:bf:23:c6");
        assert_eq!(info.get("Ssid").unwrap(), "Haven");
    }
}
