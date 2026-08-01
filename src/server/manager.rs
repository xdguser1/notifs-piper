use std::collections::{HashSet, VecDeque};
use std::fs;
use std::io::{self, ErrorKind};
use std::sync::Arc;

use tokio::net::UnixStream;

use crate::utils::{ds::ImplicationWrapper, logger::Logger};

use super::dbus::NotificationEvent;
use super::jobs::{FlagsRepr, SyncList};
use super::transmission::{FulfilledTransmission, Payload, Transmission};

pub type Watcher = ImplicationWrapper<u32, FlagsRepr>;
pub type LogCountType = u32;

pub enum ExecState {
    Executed,
    Noop,
    Error,
}

pub enum LogsDBErrorType {
    ReadPathError(io::Error),
    ParseError(serde_json::error::Error),
}

pub struct LogsConfig {
    pub max_logs: LogCountType,
}

// May be out of date with what is in logs_path
// Which means ==> NOT ACID COMPLIANT
pub struct LogsManager {
    pub(super) active_processes: HashSet<Watcher>,
    sync_list: SyncList,
    listener_path: String,
    logs_path: String,
    logs_buffer: VecDeque<NotificationEvent>,
    logs_config: LogsConfig,
}

impl LogsManager {
    fn parse_logs(path: &str) -> Result<VecDeque<NotificationEvent>, LogsDBErrorType> {
        serde_json::from_str(
            fs::read_to_string(path)
                .map_err(|err| LogsDBErrorType::ReadPathError(err))?
                .as_str(),
        )
        .map_err(|err| LogsDBErrorType::ParseError(err))
    }

    async fn send(&self, ful: FulfilledTransmission) -> io::Result<()> {
        let us = UnixStream::connect(&self.listener_path).await?;
        let stg = Transmission::new(0, Box::new(ful)).to_string();
        let mut trb = stg.as_bytes();

        loop {
            us.writable().await?;

            match us.try_write(trb) {
                Ok(n) => {
                    if n == 0 && trb.is_empty() {
                        break;
                    }

                    trb = &trb[n..];
                }
                Err(err) if err.kind() == ErrorKind::WouldBlock => {
                    continue;
                }
                Err(err) => {
                    return Err(err);
                }
            }
        }

        Ok(())
    }

    pub fn new(
        sync_list: &SyncList,
        listener_path: &str,
        logs_path: &str,
        logs_config: LogsConfig,
    ) -> LogsManager {
        let buffer = match LogsManager::parse_logs(&logs_path) {
            Ok(buf) => buf,
            Err(ref err) if let LogsDBErrorType::ReadPathError(err) = err => {
                if err.kind() == ErrorKind::NotFound {
                    Logger::info(
                        format!(
                            concat!(
                                "Previous database in '{}' could not be found.",
                                "This will create a new file once a notification is send.",
                            ),
                            logs_path,
                        )
                        .as_str(),
                    );
                } else {
                    Logger::error(
                        format!(
                            concat!(
                                "An error was found when trying to open '{}'.",
                                "This process will continue, but has a high chance of crashing once a notification is created.",
                                "Error type: {}",
                            ),
                            logs_path,
                            err.kind(),
                        ).as_str()
                    )
                }
                VecDeque::new()
            }
            Err(ref err) if let LogsDBErrorType::ParseError(err) = err => {
                Logger::error(
                    format!(
                        concat!("Logs in path '{}' could not be parsed.", "Error type: {}",),
                        logs_path, err,
                    )
                    .as_str(),
                );

                Logger::warn(
                    format!(
                        "Data in '{0}' moved to '{0}.backup' as '{0}' is used for logs database...",
                        logs_path,
                    )
                    .as_str(),
                );

                if let Err(err) = fs::rename(logs_path, format!("{}.backup", logs_path)) {
                    Logger::error(
                        format!(
                            concat!(
                                "Could not make backup of '{}'.",
                                "This is highly improbable and likely means this process will crash.",
                                "To not damage the previous logs and for debugging purposes, this process will panic.",
                                "Error type: {}",
                            ),
                            logs_path,
                            err,
                        ).as_str()
                    );
                    panic!();
                }

                if let Err(err) = fs::write(logs_path, b"[]") {
                    Logger::error(
                        format!(
                            concat!(
                                "!!FATAL ERROR!! Could not write to '{}' the logs. Exiting.",
                                "Error type: {}",
                            ),
                            logs_path, err,
                        )
                        .as_str(),
                    );
                    panic!();
                }

                VecDeque::new()
            }
            _ => {
                unreachable!()
            }
        };

        LogsManager {
            active_processes: HashSet::new(),
            sync_list: Arc::clone(sync_list),
            listener_path: listener_path.to_owned(),
            logs_path: logs_path.to_owned(),
            logs_buffer: buffer,
            logs_config,
        }
    }

    // Warning: we have methods that update the logs buffer, but where this is not called
    // automatically. This is for performance reasons. Someone who uses this function should
    // probably call `write_logs` afterwards.
    pub(super) fn append_notification(&mut self, notif: NotificationEvent) {
        if self.logs_buffer.len() == self.logs_config.max_logs as usize {
            self.logs_buffer.pop_back();
        }

        self.logs_buffer.push_front(notif);
    }

    pub(super) fn write_logs(&mut self) -> io::Result<()> {
        fs::write(
            &self.logs_path,
            serde_json::to_string(&self.logs_buffer).unwrap().as_bytes(),
        )
    }

    // Warning: we have methods that update the logs buffer, but where this is not called
    // automatically. This is for performance reasons. Someone who uses this function should
    // probably call `write_logs` afterwards.
    pub(super) fn read_logs(
        &mut self,
        start: LogCountType,
        end: LogCountType,
        update_read: bool,
    ) -> &[NotificationEvent] {
        // Should panic if called with start > end. The panicking is left to the compiler
        let slice = &mut self.logs_buffer.make_contiguous()[(start as usize)..(end as usize)];

        if update_read {
            slice.iter_mut().for_each(|el| {
                el.read = true;
            });
        }

        slice
    }

    pub fn exec(&mut self) -> io::Result<ExecState> {
        let mut vd = match self.sync_list.lock() {
            Ok(vd) => vd,
            Err(poison) => {
                Logger::error(
                    format!(
                        "{}\nError type: {}",
                        "!!FATAL ERROR!! A thread panicked while holding the lock. Exiting.",
                        poison.to_string(),
                    )
                    .as_str(),
                );
                panic!();
            }
        };

        let Some(jd) = vd.pop_front() else {
            return Ok(ExecState::Noop);
        };

        drop(vd);

        let ft = FulfilledTransmission::new(jd.desc.pid, jd.cmd.execute(&jd.desc, self));

        let err = ft.fulfilled.0.is_err();

        tokio::runtime::Runtime::new()?.block_on(self.send(ft))?;

        Ok(if err {
            ExecState::Error
        } else {
            ExecState::Executed
        })
    }
}
