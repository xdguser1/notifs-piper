use std::collections::VecDeque;
use std::fs;
use std::io::ErrorKind;
use std::sync::{Arc, Mutex, mpsc::channel};
use std::thread;

use tokio::runtime::Builder;
use zbus::connection;

use crate::utils::logger::Logger;
use dbus::{Notifications, NotificationsWrapper};
use jobs::SyncList;
use listener::Listener;
use manager::{ExecState, LogsConfig, LogsManager};

pub mod dbus;
pub mod jobs;
pub mod listener;
pub mod manager;
pub mod transmission;

pub struct ServerConfig {
    pub listener_path: String,
    pub logs_path: String,
    pub logs_config: LogsConfig,
}

pub fn start(config: ServerConfig) -> Result<!, zbus::Error> {
    let list: SyncList = Arc::new(Mutex::new(VecDeque::new()));
    let (snd, recv) = channel::<()>();

    let listener_rt = Builder::new_multi_thread()
        .worker_threads(3)
        .enable_io()
        .build()
        .expect("Cannot build listener threads.");

    let dbus_rt = Builder::new_current_thread()
        .enable_io()
        .build()
        .expect("Cannot build main runtime.");

    let listener = Listener::new(
        config.listener_path.as_str(),
        Arc::clone(&list),
        snd.clone(),
    );

    let mut manager = LogsManager::new(
        Arc::clone(&list),
        config.listener_path.as_str(),
        config.logs_path.as_str(),
        config.logs_config,
    );

    manager.notifyer = Some(snd.clone());

    let notif = NotificationsWrapper {
        inner: Notifications::new(
            // Note to self: the counter is incremented before returning the result. So
            // 'unwrap_or(0)' does not return Nid 0, since that would be illegal.
            manager.iter().map(|val| val.id()).max().unwrap_or(0),
            list,
            snd,
        ),
    };

    Logger::cdebug(
        "Setting dbus server on org.freedesktop.Notifications.",
        None,
    );
    manager.interface = Some(
        dbus_rt.block_on(
            connection::Builder::session()?
                .replace_existing_names(false)
                .name("org.freedesktop.Notifications")?
                .serve_at("/org/freedesktop/Notifications", notif)?
                .build(),
        )?,
    );

    Logger::cdebug("Removing previous socket.", None);
    match fs::remove_file(&config.listener_path) {
        Ok(_) => {}
        Err(err) if err.kind() == ErrorKind::NotFound => {}
        Err(err) => {
            Logger::error(
                format!("Could not remove old socket.\nReason: {}", err.to_string()).as_str(),
            );
            panic!();
        }
    };

    Logger::cdebug("Starting listener.", None);
    let job = listener_rt.spawn(async move { listener.listen().await.unwrap() });

    Logger::cdebug("Creating manager thread.", None);
    thread::spawn(move || {
        Logger::cdebug("Manager thread created.", None);
        for _ in recv.iter() {
            Logger::cdebug("(MANAGER THREAD): Received signal to execute job.", None);
            match manager.exec() {
                Ok(ex) => {
                    match ex {
                        ExecState::Executed => {
                            Logger::cdebug("(MANAGER THREAD): Job executed successfully.", None);
                        }
                        ExecState::Error => {
                            // The error was sent back in exec. No need to print to stderr
                            Logger::info(
                                "(MANAGER THREAD): An error occurred while processing a request.",
                            );
                        }
                        ExecState::Noop => {
                            unreachable!(
                                "This should never happen. Exec is only activated once the sync list has a job."
                            );
                        }
                    }
                }
                Err(err) => {
                    Logger::error(
                        format!(
                            concat!(
                                "!!FATAL ERROR!! An error occurred during the setup of execution.\n",
                                "Error type: {}",
                            ),
                            err.to_string()
                        )
                        .as_str(),
                    );
                    panic!();
                }
            }
        }
    });

    // Should never finish if everything went correctly
    #[allow(unused)]
    dbus_rt.block_on(job);

    unreachable!("Either infinite loop or `return Err` comes before this.");
}
