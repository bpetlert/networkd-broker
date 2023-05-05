use std::path::PathBuf;

use clap::Parser;

use crate::script::DEFAULT_TIMEOUT;

#[derive(PartialEq, Eq, Debug, Parser)]
#[command(author, version, about, long_about = None)]
pub struct Arguments {
    /// Location under which to look for scripts
    #[arg(
        short = 'S',
        long = "script-dir",
        default_value = "/etc/networkd/broker.d"
    )]
    pub script_dir: PathBuf,

    /// Generate events reflecting preexisting state and behavior on startup
    #[arg(short = 'T', long = "startup-triggers")]
    pub startup_triggers: bool,

    /// Script execution timeout in seconds
    #[arg(short = 't', long = "timeout", default_value_t = DEFAULT_TIMEOUT)]
    pub timeout: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::{CommandFactory, FromArgMatches};

    #[test]
    fn test_args() {
        // Default arguments
        let args = Arguments::from_arg_matches(
            &Arguments::command().get_matches_from(vec![env!("CARGO_CRATE_NAME")]),
        )
        .expect("Paring argument");
        assert_eq!(args.script_dir, PathBuf::from("/etc/networkd/broker.d"));
        assert!(!args.startup_triggers);
        assert_eq!(args.timeout, DEFAULT_TIMEOUT);

        // Full long arguments
        let args = Arguments::from_arg_matches(&Arguments::command().get_matches_from(vec![
            env!("CARGO_CRATE_NAME"),
            "--script-dir",
            "/etc/networkd/broker2.d",
            "--startup-triggers",
            "--timeout",
            "50",
        ]))
        .expect("Paring argument");
        assert_eq!(args.script_dir, PathBuf::from("/etc/networkd/broker2.d"));
        assert!(args.startup_triggers);
        assert_eq!(args.timeout, 50);

        // Full short arguments
        let args = Arguments::from_arg_matches(&Arguments::command().get_matches_from(vec![
            env!("CARGO_CRATE_NAME"),
            "-S",
            "/etc/networkd/broker2.d",
            "-T",
            "-t",
            "50",
        ]))
        .expect("Paring argument");
        assert_eq!(args.script_dir, PathBuf::from("/etc/networkd/broker2.d"));
        assert!(args.startup_triggers);
        assert_eq!(args.timeout, 50);
    }
}
