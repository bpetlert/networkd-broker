use structopt::StructOpt;

#[derive(StructOpt, PartialEq, Debug)]
#[structopt(name = "networkd-broker", about = "networkd event broker daemon")]
pub struct Arguments {
    /// Location under which to look for scripts
    #[structopt(
        short = "S",
        long = "script-dir",
        default_value = "/etc/networkd/broker.d"
    )]
    pub script_dir: String,

    /// Generate events reflecting preexisting state and behavior on startup
    #[structopt(short = "T", long = "run-startup-triggers")]
    pub run_startup_triggers: bool,

    /// Script execution timeout in seconds
    #[structopt(short = "t", long = "timeout", default_value = "20")]
    pub timeout: u64,

    /// Pass JSON encoding of event and link status to script
    #[structopt(short = "j", long = "json")]
    pub json: bool,

    /// Increment verbosity level once per call
    /// [error, -v: warn, -vv: info, -vvv: debug, -vvvv: trace]
    #[structopt(short = "v", long = "verbose", parse(from_occurrences))]
    pub verbose: u8,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_args() {
        // Default arguments
        assert_eq!(
            Arguments {
                script_dir: "/etc/networkd/broker.d".to_owned(),
                run_startup_triggers: false,
                timeout: 20,
                json: false,
                verbose: 0
            },
            Arguments::from_clap(&Arguments::clap().get_matches_from(&["test",]))
        );

        // Full long arguments
        assert_eq!(
            Arguments {
                script_dir: "/etc/networkd/broker2.d".to_owned(),
                run_startup_triggers: true,
                timeout: 50,
                json: true,
                verbose: 4
            },
            Arguments::from_clap(&Arguments::clap().get_matches_from(&[
                "test",
                "--script-dir",
                "/etc/networkd/broker2.d",
                "--run-startup-triggers",
                "--timeout",
                "50",
                "--json",
                "--verbose",
                "--verbose",
                "--verbose",
                "--verbose"
            ]))
        );

        // Full short arguments
        assert_eq!(
            Arguments {
                script_dir: "/etc/networkd/broker2.d".to_owned(),
                run_startup_triggers: true,
                timeout: 50,
                json: true,
                verbose: 4
            },
            Arguments::from_clap(&Arguments::clap().get_matches_from(&[
                "test",
                "-S",
                "/etc/networkd/broker2.d",
                "-T",
                "-t",
                "50",
                "-j",
                "-vvvv"
            ]))
        );
    }
}
