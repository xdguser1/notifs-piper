#![feature(never_type)]

use std::env;
use std::fs;

use clap::Parser;
use tokio::{net::UnixStream, runtime::Builder};

use cli::{CAPABILITIES_ENUMERATED, Cli, Sub};
use server::{
    ServerConfig,
    dbus::CAPABILITIES,
    jobs::{Desc, Flags, FlagsRepr, JobDesc, Read, Watch},
    listener::Listener,
    manager::LogsConfig,
    transmission::{Payload, Transmission, TransmissionType},
};
use utils::logger::Logger;

mod cli;
mod consts;
mod server;
mod utils;

fn main() {
    let parsed = Cli::parse();
    let path = env::var("XDG_RUNTIME_DIR")
        .or(env::var("XDG_DATA_HOME"))
        .or(env::var("HOME").map(|val| val + "/.local/share"))
        .unwrap()
        + "/npiper";
    let listener_path = path.clone() + "/pipe";

    macro_rules! send_job {
        ($us:ident, $job:expr, $($flags:expr)*; $($rest:tt)*) => {
             Builder::new_current_thread()
                .enable_io()
                .build()
                .expect("Could not build async runtime.")
                .block_on(async move {
                    let Ok($us) = UnixStream::connect(&listener_path).await else {
                        Logger::error(format!("Could not connect to server through '{}'", listener_path).as_str());
                        Logger::info("Check if the notifs-piper daemon is enabled.");
                        return;
                    };

                    let pid = std::process::id();

                    Logger::cdebug("Sending new transmission...", None);
                    if let Err(err) = Listener::write(
                        &$us,
                        &Transmission::new(
                            TransmissionType::Incoming(pid),
                            JobDesc::new(
                                Box::new($job),
                                Desc::new(
                                    pid,
                                    {
                                        let mut fcon = Flags::NONE as FlagsRepr;
                                        $(
                                            fcon = Flags::join(
                                                fcon,
                                                $flags
                                            );
                                        )*

                                        fcon
                                    }
                                )
                            ).to_string(),
                        )
                    ).await {
                        Logger::error(format!("Could not connect to server.\nReason: {}", err.to_string()).as_str());
                        return;
                    }
                    Logger::cdebug("Transmission sent.", None);

                    $($rest)*
                });
        };
    }

    Logger::cdebug("~~ Debugging session ~~", Some(parsed.debug));

    match parsed.subcommand {
        Sub::Daemon {
            logs_file,
            max,
            options,
            all,
        } => {
            Logger::cdebug(format!("Running daemon in '{}'.", path).as_str(), None);

            fs::create_dir_all(&path)
                .inspect_err(|_| Logger::error("Internal error: could not create data directory."))
                .unwrap();

            let logs_path: String;

            if let Some(pb) = logs_file.as_ref().map(|val| val.as_path()) {
                if !pb.is_file() {
                    Logger::error("'logs-file' is not a valid file.");
                    return;
                }

                logs_path = pb.to_str().unwrap().to_owned();
            } else {
                logs_path = path.clone() + "/logs.json";
            }

            if all {
                CAPABILITIES.set(CAPABILITIES_ENUMERATED.to_vec()).unwrap();
            } else {
                CAPABILITIES
                    .set(
                        options
                            .into_iter()
                            .map(|val| val.leak::<'static>() as &'static str)
                            .collect(),
                    )
                    .unwrap();
            }

            let Err(err) = server::start_server(ServerConfig {
                listener_path,
                logs_path,
                logs_config: LogsConfig { max_logs: max },
            });

            Logger::error(
                format!(
                    concat!(
                        "Could not start server on org.freedesktop.Notifications.\n",
                        "Error type: {}",
                    ),
                    err.to_string(),
                )
                .as_str(),
            );
        }
        Sub::Read {
            count,
            skip,
            silent,
        } => {
            send_job!(
                us,
                Read::new(skip, skip + count),
                if silent { Flags::SILENT } else { Flags::NONE };
                /*---------------------------------------------*/
                match Listener::read(&us).await {
                    Ok(tr) => {
                        println!("{}", tr.data);
                    },
                    Err(err) => {
                        Logger::error(format!("Could not connect to server.\nReason: {}", err.to_string()).as_str());
                    },
                };
            );
        }
        Sub::Watch { silent } => {
            send_job!(
                us,
                Watch,
                if silent { Flags::SILENT } else { Flags::NONE };
                /*---------------------------------------------*/
                while let Ok(tr) = Listener::read(&us).await {
                    println!("{}", tr.data);
                }
                Logger::error("Could not connect to server. Check daemon logs for more information.");
            );
        }
    }
}
