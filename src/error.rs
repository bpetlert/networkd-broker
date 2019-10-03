use dbus::message::{Message, MessageType};
use std::{error::Error as StdError, fmt, path::Path};

#[derive(Debug, PartialEq)]
pub enum ErrorKind {
    /// Generic error
    Msg(String),

    /// Invoke 'iw' command fails
    CallIwFailed(String),

    /// 'iw' command failled with 'No such device (-19)'
    LinkNotExist(String),

    /// iface does not connect to an access point.
    NotConnected(String),

    /// Cannot parse output from 'iw' command
    ParseIwLinkFailed(String),

    /// Invoke 'networkctl' command fails
    CallNetworkctlFailed(String),

    /// D-Bus message is not signal
    NotDBusSignal(MessageType),

    /// D-Bus interface is not 'org.freedesktop.DBus.Properties'
    NotDBusProperties(String),

    /// Event is not 'org.freedesktop.network1.Link'
    NotLinkEvent(String),

    /// Invalid AdministrativeState/OperationalState
    InvalidStateType(String),

    InvalidOperationalStatus(String),

    CannotConvertEventMessage(String),

    LinkToIndex(String),

    /// Script directory does not exist
    PathNotExist(String),

    /// Cannot execute script
    ExecuteFailed(String),

    /// No script found in given directory.
    NoScriptFound(String),

    ExecuteTimeout(String),
}

#[derive(Debug)]
pub struct Error {
    pub kind: ErrorKind,
    source: Option<Box<dyn StdError>>,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.kind {
            ErrorKind::Msg(ref message) => fmt::Display::fmt(message, f),
            ErrorKind::CallIwFailed(ref e) => fmt::Display::fmt(e, f),
            ErrorKind::LinkNotExist(ref e) => fmt::Display::fmt(e, f),
            ErrorKind::NotConnected(ref e) => fmt::Display::fmt(e, f),
            ErrorKind::ParseIwLinkFailed(ref e) => fmt::Display::fmt(e, f),
            ErrorKind::CallNetworkctlFailed(ref e) => fmt::Display::fmt(e, f),
            ErrorKind::NotDBusSignal(ref m) => fmt::Display::fmt(&format!("{:?}", m), f),
            ErrorKind::NotDBusProperties(ref e) => fmt::Display::fmt(e, f),
            ErrorKind::NotLinkEvent(ref e) => fmt::Display::fmt(e, f),
            ErrorKind::InvalidStateType(ref e) => fmt::Display::fmt(e, f),
            ErrorKind::InvalidOperationalStatus(ref e) => fmt::Display::fmt(e, f),
            ErrorKind::CannotConvertEventMessage(ref e) => fmt::Display::fmt(e, f),
            ErrorKind::LinkToIndex(ref e) => fmt::Display::fmt(e, f),
            ErrorKind::PathNotExist(ref e) => fmt::Display::fmt(e, f),
            ErrorKind::ExecuteFailed(ref e) => fmt::Display::fmt(e, f),
            ErrorKind::NoScriptFound(ref e) => fmt::Display::fmt(e, f),
            ErrorKind::ExecuteTimeout(ref e) => fmt::Display::fmt(e, f),
        }
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        self.source.as_ref().map(|c| &**c)
    }
}

impl Error {
    /// Creates generic error
    pub fn msg(value: impl ToString) -> Self {
        Self {
            kind: ErrorKind::Msg(value.to_string()),
            source: None,
        }
    }

    /// Creates iw command error
    pub fn call_iw_failed(value: impl ToString) -> Self {
        Self {
            kind: ErrorKind::CallIwFailed(value.to_string()),
            source: None,
        }
    }

    pub fn link_not_exist(value: impl ToString) -> Self {
        Self {
            kind: ErrorKind::LinkNotExist(format!("{} does not exist.", value.to_string())),
            source: None,
        }
    }

    pub fn not_connected(value: impl ToString) -> Self {
        Self {
            kind: ErrorKind::NotConnected(format!(
                "{} does not connect to an access point.",
                value.to_string()
            )),
            source: None,
        }
    }

    pub fn parse_iw_link_failed(value: impl ToString) -> Self {
        Self {
            kind: ErrorKind::ParseIwLinkFailed(value.to_string()),
            source: None,
        }
    }

    pub fn call_networkctl_failed(value: impl ToString) -> Self {
        Self {
            kind: ErrorKind::CallNetworkctlFailed(value.to_string()),
            source: None,
        }
    }

    pub fn not_dbus_signal(value: MessageType) -> Self {
        Self {
            kind: ErrorKind::NotDBusSignal(value),
            source: None,
        }
    }

    pub fn not_dbus_properties(value: impl ToString) -> Self {
        Self {
            kind: ErrorKind::NotDBusProperties(value.to_string()),
            source: None,
        }
    }

    pub fn not_link_event(value: impl ToString) -> Self {
        Self {
            kind: ErrorKind::NotLinkEvent(value.to_string()),
            source: None,
        }
    }

    pub fn invalid_state_type(value: impl ToString) -> Self {
        Self {
            kind: ErrorKind::InvalidStateType(value.to_string()),
            source: None,
        }
    }

    pub fn invalid_operational_status(value: impl ToString) -> Self {
        Self {
            kind: ErrorKind::InvalidOperationalStatus(value.to_string()),
            source: None,
        }
    }

    pub fn cannot_convert_event_message(value: &Message) -> Self {
        Self {
            kind: ErrorKind::CannotConvertEventMessage(format!("{:?}", value)),
            source: None,
        }
    }

    pub fn link_to_index(value: impl ToString) -> Self {
        Self {
            kind: ErrorKind::LinkToIndex(value.to_string()),
            source: None,
        }
    }

    pub fn path_not_exist(path: &Path) -> Self {
        Self {
            kind: ErrorKind::PathNotExist(format!("Path does not exist: {:?}", path)),
            source: None,
        }
    }

    pub fn execute_failed(value: impl ToString) -> Self {
        Self {
            kind: ErrorKind::ExecuteFailed(value.to_string()),
            source: None,
        }
    }

    pub fn no_script_found(path: &Path) -> Self {
        Self {
            kind: ErrorKind::NoScriptFound(format!("No script found in: {:?}", path)),
            source: None,
        }
    }

    pub fn execute_timeout(secs: u64, path: &Path) -> Self {
        Self {
            kind: ErrorKind::ExecuteTimeout(format!("Execute timeout: >= {}, {:?}", secs, &path)),
            source: None,
        }
    }
}

impl From<&str> for Error {
    fn from(e: &str) -> Self {
        Self::msg(e)
    }
}

impl From<String> for Error {
    fn from(e: String) -> Self {
        Self::msg(e)
    }
}

pub type Result<T> = ::std::result::Result<T, Error>;
