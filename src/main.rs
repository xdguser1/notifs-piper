#![feature(never_type)]

use std::env;
use std::fmt::Display;
use std::fs;
use std::io;
use std::path::Path;

use clap::Parser;
use tokio::net::UnixStream;

use cli::{Cli, SignalKind, Sub};
use consts::CAPABILITIES_ENUMERATED;
use server::{
    ServerConfig,
    dbus::CAPABILITIES,
    jobs::{
        ActionInvoked, ActivationToken, Close, Desc, Flags, FlagsRepr, Job, JobDesc,
        NotificationClosed, Pid, Query, Read, Watch,
    },
    listener::Listener,
    manager::LogsConfig,
    transmission::{Payload, Transmission, TransmissionType},
};
use utils::{
    logger::Logger,
    macros::{async_rt::block_on_io, utilities::expand_option},
};

mod cli;
mod consts;
mod server;
mod utils;

macro_rules! send {
    ($path:ident, $job:expr, $($flags:expr),*; $($pid:literal)?) => {
        {
            let Ok(stream) = connect($path).await else { return; };

            #[allow(unused_mut)]
            let mut flags: FlagsRepr = Flags::NONE as FlagsRepr;
            $(
                flags = Flags::join(flags, $flags);
            )*

            let Ok(_) = send(&stream, $job, flags, expand_option!($($pid)?)).await else { return; };

            stream
        }
    };
}

macro_rules! send_once {
    ($path:ident, $job:expr, $($flags:expr),*; $($pid:literal)?) => {
        {
            let stream = send!($path, $job, $($flags)*; $($pid)?);

            let Ok(trans) = read(&stream).await else { return; };

            trans
        }
    };
}

macro_rules! send_once_and_print {
    ($path:ident, $job:expr, $($flags:expr),*; $($pid:literal)?) => {
        {
            let trans = send_once!($path, $job, $($flags)*; $($pid)?);
            println!("{}", trans.to_string());
        }
    };
}

async fn connect<P>(path: P) -> io::Result<UnixStream>
where
    P: AsRef<Path> + Display,
{
    match UnixStream::connect(&path).await {
        Ok(stream) => Ok(stream),
        Err(err) => {
            Logger::error(format!("Could not connect to server at '{}'.", path).as_str());
            Logger::info("Check if the notifs-piper daemon is enabled with 'busctl --user list'");
            Err(err)
        }
    }
}

async fn send(
    stream: &UnixStream,
    job: Box<dyn Job>,
    flags: FlagsRepr,
    pid: Option<Pid>,
) -> io::Result<()> {
    Logger::cdebug("Sending new transmission...", None);

    let pid = pid.unwrap_or_else(|| std::process::id());

    if let Err(err) = Listener::write(
        stream,
        &Transmission::new(
            TransmissionType::Incoming(pid),
            JobDesc::new(job, Desc::new(pid, flags)).to_string(),
        ),
    )
    .await
    {
        Logger::error(
            format!("Could not connect to server.\nReason: {}", err.to_string()).as_str(),
        );
        return Err(err);
    }

    Logger::cdebug("Transmission sent.", None);

    Ok(())
}

async fn read(stream: &UnixStream) -> io::Result<Transmission> {
    match Listener::read(&stream).await {
        Ok(trans) => Ok(trans),
        Err(err) => {
            Logger::error(
                format!("Could not connect to server.\nReason: {}", err.to_string()).as_str(),
            );
            Err(err)
        }
    }
}

fn main() {
    let parsed = Cli::parse();
    let path = env::var("XDG_RUNTIME_DIR")
        .or(env::var("XDG_DATA_HOME"))
        .or(env::var("HOME").map(|val| val + "/.local/share"))
        .unwrap()
        + "/npiper";
    let listener_path = path.clone() + "/pipe";

    Logger::cdebug("~~ Debugging session ~~", Some(parsed.debug));

    match parsed.subcommand {
        Sub::Signal { id, force, kind } => {
            block_on_io!(async move {
                if let SignalKind::Closed { query } = kind
                    && query
                {
                    send_once_and_print!(listener_path, Box::new(Query(id)), /* No flags */ ;);
                    return;
                }

                let job: Box<dyn Job> = match &kind {
                    SignalKind::Closed {
                        query: _, /* false */
                    } => Box::new(Close::new(id, NotificationClosed::Dismissed)) as Box<dyn Job>,
                    SignalKind::ActionInvoked { action } => {
                        Box::new(ActionInvoked::new(id, action.clone())) as Box<dyn Job>
                    }
                    SignalKind::ActivationToken { token } => {
                        Box::new(ActivationToken::new(id, token.clone())) as Box<dyn Job>
                    }
                };

                send!(listener_path, job, if force { Flags::FORCE } else { Flags::NONE }; 0);
            });
        }
        Sub::Daemon {
            logs_file,
            max,
            options,
            all,
            auto_close,
            timeout,
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

            let Err(err) = server::start(ServerConfig {
                listener_path,
                logs_path,
                logs_config: LogsConfig {
                    max_logs: max,
                    auto_close,
                    default_timeout: timeout,
                },
            });

            Logger::error(
                format!(
                    concat!(
                        "Could not start server on 'org.freedesktop.Notifications'.\n",
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
            block_on_io!(async move {
                send_once_and_print!(listener_path, Box::new(Read::new(skip, skip + count)), if silent { Flags::SILENT } else { Flags::NONE };);
            });
        }
        Sub::Watch { silent } => {
            block_on_io!(async move {
                let stream = send!(listener_path, Box::new(Watch), if silent { Flags::SILENT } else { Flags::NONE };);

                while let Ok(trans) = Listener::read(&stream).await {
                    println!("{}", trans.data);
                }
                Logger::error(
                    "Could not connect to server. Check daemon logs for more information.",
                );
            });
        }
    }
}
