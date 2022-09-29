use clap::Parser;

#[derive(PartialEq, Eq, Debug, Parser)]
#[command(author, version, about, long_about = None)]
pub struct Arguments {
    /// Location under which to look for scripts
    #[arg(
        short = 'S',
        long = "script-dir",
        default_value = "/etc/networkd/broker.d"
    )]
    pub script_dir: String,

    /// Generate events reflecting preexisting state and behavior on startup
    #[arg(short = 'T', long = "run-startup-triggers")]
    pub run_startup_triggers: bool,

    /// Script execution timeout in seconds
    #[arg(short = 't', long = "timeout", default_value = "20")]
    pub timeout: u64,
}

#[cfg(test)]
mod tests {
    use clap::{CommandFactory, FromArgMatches};

    use super::*;

    #[test]
    fn test_args() {
        // Default arguments
        let args = Arguments::from_arg_matches(
            &Arguments::command().get_matches_from(vec![env!("CARGO_CRATE_NAME")]),
        )
        .expect("Paring argument");
        assert_eq!(args.script_dir, "/etc/networkd/broker.d".to_owned());
        assert!(!args.run_startup_triggers);
        assert_eq!(args.timeout, 20);

        // Full long arguments
        let args = Arguments::from_arg_matches(&Arguments::command().get_matches_from(vec![
            env!("CARGO_CRATE_NAME"),
            "--script-dir",
            "/etc/networkd/broker2.d",
            "--run-startup-triggers",
            "--timeout",
            "50",
        ]))
        .expect("Paring argument");
        assert_eq!(args.script_dir, "/etc/networkd/broker2.d".to_owned());
        assert!(args.run_startup_triggers);
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
        assert_eq!(args.script_dir, "/etc/networkd/broker2.d".to_owned());
        assert!(args.run_startup_triggers);
        assert_eq!(args.timeout, 50);
    }
}
