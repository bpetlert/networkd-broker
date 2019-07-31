#[derive(Debug, PartialEq)]
pub enum AppError {
    CallIwlFailed,
    CallNetworkctlFailed,
    CannotConvertEventMessage,
    ExecuteFailed,
    InvalidOperationalStatus,
    InvalidStateType,
    LinkNotExist,
    LinkToIndex,
    NoPathFound,
    NoScriptFound,
    NotConnected,
    NotDBusProperties,
    NotLinkEvent,
    NotSignal,
    ParseIwLinkFailed,
    ParseNetworkctlListFailed,
    ParseNetworkctlStatusFailed,
    Timeout,
}

pub type Result<T> = std::result::Result<T, AppError>;
