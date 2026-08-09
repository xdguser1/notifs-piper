#![feature(never_type)]

use std::env;
use std::fs;

use server::{ServerConfig, manager::LogsConfig};

mod server;
mod utils;

fn main() {
    let path = env::var("XDG_RUNTIME_DIR")
        .or(env::var("XDG_DATA_HOME"))
        .or(env::var("HOME").map(|val| val + "/.local/share"))
        .unwrap()
        + "/npiper";

    fs::create_dir_all(&path).expect("Could not create data directory.");

    let config = ServerConfig {
        listener_path: path.clone() + "/pipe",
        logs_path: path.clone() + "/logs.json",
        logs_config: LogsConfig { max_logs: 1000 },
    };

    server::start_server(config);
}
