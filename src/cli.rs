use std::path::PathBuf;

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
    Watch {
        /// By default, if a process listens to the server, it will mark every notification as
        /// read. This option is to keep the notifications unread.
        #[arg(short, long, default_value_t = false)]
        silent: bool,
    },
}
