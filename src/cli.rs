use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(version, about, long_about = None)]
pub struct Cli {
    /// Allow debugging information to be shown in the output.
    #[arg(short, long, default_value_t = false)]
    pub debug: bool,

    #[command(subcommand)]
    pub subcommand: Sub,
}

#[derive(Subcommand)]
pub enum Sub {
    /// Starts the notification server. This will fail if there is another server already running
    /// org.freedesktop.Notifications.
    Daemon {
        /// Which file should be read and written for logging notifications.
        #[arg(short = 'f', long = "logs-file")]
        logs_file: Option<PathBuf>,

        /// Maximum amount of notifications kept in logs. Defaults to 100.
        #[arg(short, long, default_value_t = 100)]
        max: u16,

        /// Adds the given <OPTION> capability to the daemon notification server. By default, no capability
        /// is activated.
        /// See https://specifications.freedesktop.org/notification/latest/protocol.html#id-1.10.3.2.5
        /// for details on the possible capabilities of the server. Note that this binary supports
        /// every capability, but it is up to the user to use them correctly (ref: README on github).
        #[arg(short = 'o', long = "option", value_parser = opts_checker)]
        options: Vec<String>,

        /// Allows all the capabilities for the server. Is disabled by default.
        #[arg(short, long, default_value_t = false)]
        all: bool,
    },
    /// Reads <COUNT> notifications from the logs and returns the result in a JSON format.
    Read {
        /// The number of notifications to be read.
        count: u16,

        /// Skips the first <SKIP> notifications in the logs.
        #[arg(short, long, default_value_t = 0)]
        skip: u16,

        /// A notification may be unread if no process listened to the server. This keeps it unread.
        #[arg(short = 'q', long, default_value_t = false)]
        silent: bool,
    },
    /// Watches for new notifications, events and signals emitted by either the dbus server or
    /// other processes listening. In practice, this can be considered as a "tiny notification
    /// server" without the graphical interface (ref: README on github).
    Watch {
        /// By default, if a process listens to the server, it will mark every notification as
        /// read. This option is to keep the notifications unread.
        #[arg(short, long, default_value_t = false)]
        silent: bool,
    },
}

pub const CAPABILITIES_ENUMERATED: [&'static str; 10] = [
    "action-icons",
    "actions",
    "body",
    "body-hyperlinks",
    "body-images",
    "body-markup",
    "icon-multi",
    "icon-static",
    "persistence",
    "sound",
];

fn opts_checker(val: &str) -> Result<String, String> {
    static MUTUALLY_EXCLUDED_ICON: AtomicBool = AtomicBool::new(false);

    if val.starts_with("icon") {
        if MUTUALLY_EXCLUDED_ICON.load(Ordering::Relaxed) {
            panic!(concat!(
                "'icon-static' and 'icon-multi' are mutually exclusive.",
                "Please refer to the docs from freedesktop.org for the Notifications interface."
            ));
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
