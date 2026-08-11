use std::collections::VecDeque;
use std::fs;
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

pub fn start_server(config: ServerConfig) -> Result<!, zbus::Error> {
    let sync_list: SyncList = Arc::new(Mutex::new(VecDeque::new()));
    let (snd, recv) = channel::<()>();

    let mut manager = LogsManager::new(
        &sync_list,
        config.listener_path.as_str(),
        config.logs_path.as_str(),
        config.logs_config,
    );

    let notif = NotificationsWrapper {
        inner: Notifications::new(
            manager.iter().map(|not| not.get_id()).max().unwrap_or(0),
            &sync_list,
            snd.clone(),
        ),
    };

    let listener = Listener::new(config.listener_path.as_str(), &sync_list, snd);

    let con = connection::Builder::session()?
        .replace_existing_names(false)
        .name("org.freedesktop.Notifications")?
        .serve_at("/org/freedesktop/Notifications", notif)?
        .build();

    Logger::cdebug("Removing previous pipe.", None);

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
                            Logger::info("An error occurred while processing a request.");
                        }
                        ExecState::Noop => {
                            unreachable!(
                                "This should never happen. Exec is only activated once sync_list has a job."
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
                            err
                        )
                        .as_str(),
                    );
                    panic!();
                }
            }
        }
    });

    let handle = Builder::new_multi_thread()
        .worker_threads(3)
        .enable_io()
        .build()
        .expect("Cannot build listener threads.");

    if let Ok(runtime) = Builder::new_current_thread().enable_io().build() {
        let _con = runtime.block_on(con)?;
        let _ = fs::remove_file(&config.listener_path);
        let job = handle.spawn(async move { listener.listen().await.unwrap() });

        // Should never finish if everything went correctly
        #[allow(unused)]
        runtime.block_on(job);
    }

    unreachable!("Either infinite loop or `return Err` comes before this.");
}
