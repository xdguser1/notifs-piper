use std::cmp::min;
use std::collections::VecDeque;
use std::fs;
use std::io::{self, ErrorKind};
use std::sync::Arc;

use tokio::net::UnixStream;
use zbus::Connection;

use crate::utils::logger::Logger;

use super::dbus::{Nid, NotificationEvent};
use super::jobs::{FulfilledJob, Pid, SyncList};
use super::transmission::{Payload, Transmission};

pub type LogCountType = u16;

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

// There is no guarantee that logs_buffer is synchronised with
// the logs_path file. This is not ACID compliant.
pub struct LogsManager {
    sync_list: SyncList,
    listener_path: String,
    logs_path: String,
    logs_buffer: VecDeque<NotificationEvent>,
    logs_config: LogsConfig,
    dirty: bool,
    pub(super) interface: Option<Connection>,
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

    async fn send(&self, ful: &FulfilledJob, pid: Pid) -> io::Result<()> {
        let us = UnixStream::connect(&self.listener_path).await?;
        let stg = Payload::to_string(&Transmission::from_fulfilled(ful, pid));
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
                                "Previous database in '{}' could not be found.\n",
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
                                "An error was found when trying to open '{}'.\n",
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
                        "Logs in path '{}' could not be parsed.\nError type: {}",
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
                                "Could not make backup of '{}'.\n",
                                "This is highly improbable and likely means this process will crash.\n",
                                "To not damage the previous logs and for debugging purposes, this process will panic.\n",
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
                                "!!FATAL ERROR!! Could not write to '{}' the logs. Exiting.\n",
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
            sync_list: Arc::clone(sync_list),
            listener_path: listener_path.to_owned(),
            logs_path: logs_path.to_owned(),
            logs_buffer: buffer,
            logs_config,
            dirty: false,
            interface: None,
        }
    }

    pub(super) fn iter<'a>(&'a self) -> impl Iterator<Item = &'a NotificationEvent> {
        self.logs_buffer.iter()
    }

    pub(super) fn remove_notification(&mut self, nid: Nid) -> Option<NotificationEvent> {
        let pos = self.logs_buffer.iter().position(|val| val.get_id() == nid);
        if pos.is_none() {
            return None;
        }
        self.dirty = true;
        self.logs_buffer.swap_remove_back(pos.unwrap())
    }

    pub(super) fn append_notification(&mut self, notif: NotificationEvent) {
        if self.logs_buffer.len() == self.logs_config.max_logs as usize {
            self.logs_buffer.pop_back();
        }

        self.logs_buffer.push_front(notif);
        self.dirty = true;
    }

    pub(super) fn read_notification(&mut self, not: Nid) {
        // PERF: Since the notifications are appended in the front and most notifications are
        // marked as "read" the moment they are sent, this will usually run with only 1 iteration.
        // Note though, that logs_buffer has no guarantee to be ordered, so the worst case scenario
        // is still O(n).
        self.logs_buffer
            .iter_mut()
            .find(|val| val.get_id() == not)
            .map(|not| {
                not.read = true;
            });
        self.dirty = true;
    }

    pub(super) fn read_logs(
        &mut self,
        start: LogCountType,
        end: LogCountType,
        update_read: bool,
    ) -> &[NotificationEvent] {
        let len = self.logs_buffer.len();

        let slice = &mut self.logs_buffer.make_contiguous()
            [(min(len, start as usize))..(min(len, end as usize))];

        let len = slice.len();
        if update_read && len != 0 {
            self.dirty = true;
            slice.iter_mut().for_each(|el| {
                el.read = true;
            });
            Logger::cdebug(
                format!("(MANAGER THREAD): Marking {} notifications read.", len).as_str(),
                None,
            );
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
            Logger::cdebug("(MANAGER THREAD): No job executed.", None);
            return Ok(ExecState::Noop);
        };

        let empty = vd.is_empty();

        drop(vd);

        let fj = jd.cmd.execute(&jd.desc, self);
        Logger::cdebug("(MANAGER THREAD): Job executed.", None);

        if empty && self.dirty {
            Logger::cdebug("(MANAGER THREAD): Writing results in logs.", None);
            self.dirty = false;
            fs::write(
                &self.logs_path,
                serde_json::to_string(&self.logs_buffer).unwrap().as_bytes(),
            )?;
            Logger::cdebug("(MANAGER THREAD): Logs written.", None);
        }

        if fj.result.as_ref().is_ok_and(|v| v.is_none()) {
            Logger::cdebug("(MANAGER THREAD): Nothing to send back.", None);
            return Ok(ExecState::Executed);
        }

        Logger::cdebug("(MANAGER THREAD): Sending back results.", None);
        tokio::runtime::Builder::new_current_thread()
            .enable_io()
            .build()?
            .block_on(self.send(&fj, jd.desc.pid))?;

        Ok(if fj.result.is_err() {
            Logger::cdebug("(MANAGER THREAD): Error in last job execution.", None);
            ExecState::Error
        } else {
            ExecState::Executed
        })
    }
}
