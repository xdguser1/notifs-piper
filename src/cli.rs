use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

use clap::{Parser, Subcommand};

use crate::consts::CAPABILITIES_ENUMERATED;
use crate::server::dbus::Nid;

#[derive(Parser)]
#[command(version, about, long_about = None)]
pub struct Cli {
    /// Allow debugging information to be shown in the output.
    #[arg(short, long, default_value_t = false)]
    pub debug: bool,

    /// The command to be run.
    #[command(subcommand)]
    pub subcommand: Sub,
}

#[derive(Subcommand)]
pub enum Sub {
    /// Starts the notification server. There can only be one.
    Daemon {
        /// File to store and read notification logs.
        #[arg(short = 'f', long = "logs-file")]
        logs_file: Option<PathBuf>,

        /// Maximum amount of notifications kept in logs.
        #[arg(short, long, default_value_t = 100)]
        max: u16,

        /// Automatically closes a notification after timeout.
        #[arg(short = 'c', long, default_value_t = false)]
        auto_close: bool,

        /// Adds a capability to the server. By default, no capability is activated.
        #[arg(short = 'o', long = "option", value_parser = opts_checker)]
        options: Vec<String>,

        /// Adds all capabilities for the server. See procotol for the list.
        #[arg(short, long, default_value_t = false)]
        all: bool,

        /// Default timeout option for expire_timeout being -1.
        #[arg(short, long, default_value_t = 5000)]
        timeout: u16,
    },
    /// Reads a number of notifications from the logs.
    Read {
        /// The number of notifications to be read.
        count: u16,

        /// Skips an amount of notifications in the logs before reading.
        #[arg(short, long, default_value_t = 0)]
        skip: u16,

        /// Notifications won't be marked as read from this.
        #[arg(short = 'q', long, default_value_t = false)]
        silent: bool,
    },
    /// Pipes notifications and events to the output.
    Watch {
        /// Notifications won't be marked as read from this.
        #[arg(short = 'q', long, default_value_t = false)]
        silent: bool,
    },
    /// Signals a signal to the notification server.
    Signal {
        /// The notification id on which the signal was emitted.
        ///
        /// Note that, by default, a notification may be valid longer than what this
        /// program perceives as valid (e.g. *max* option set to 10 on the daemon,
        /// but the user's program stores 11 notifications in memory).
        ///
        /// As such, to preserve validity, this will only execute if:
        ///
        /// (1) the notification isn't previously closed in the logs and
        ///
        /// (2) the notification is stored in the logs
        ///
        /// To force the signal to be emitted, use --force option, however,
        /// signaling a closed notification will lead to undefined behaviour
        /// as it is breaking the official protocol.
        ///
        /// No guarantees are made relating to the signal ordering.
        id: Nid,

        /// Forces the signal to be emitted.
        #[arg(short, long, default_value_t = false)]
        force: bool,

        /// The signal kind that will be sent.
        #[command(subcommand)]
        kind: SignalKind,
    },
}

#[derive(Subcommand, PartialEq, Eq)]
pub enum SignalKind {
    /// Signals that the user has closed the notification.
    Closed {
        /// Queries whether a notification is currently closed.
        #[arg(short, long, default_value_t = false)]
        query: bool,
    },
    /// Signals an action to be invoked.
    ActionInvoked { action: String },
    /// An activation token. See protocol for more info.
    ActivationToken { token: String },
}

fn opts_checker(val: &str) -> Result<String, String> {
    static MUTUALLY_EXCLUDED_ICON: AtomicBool = AtomicBool::new(false);

    if val.starts_with("icon") {
        if MUTUALLY_EXCLUDED_ICON.load(Ordering::Relaxed) {
            return Err(concat!(
                "'icon-static' and 'icon-multi' are mutually exclusive.",
                "Please refer to the docs from freedesktop.org for the Notifications interface."
            )
            .to_string());
        }

        MUTUALLY_EXCLUDED_ICON.store(true, Ordering::Relaxed);
    }

    if CAPABILITIES_ENUMERATED.contains(&val) {
        Ok(val.to_owned())
    } else {
        Err(format!(
            "Invalid capability. Possible values are:\n{}",
            CAPABILITIES_ENUMERATED.join("\n")
        ))
    }
}
