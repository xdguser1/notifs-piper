use std::cmp::min;
use std::collections::VecDeque;
use std::fs;
use std::io::{self, ErrorKind};
use std::sync::{Arc, mpsc::Sender};

use tokio::net::UnixStream;
use zbus::Connection;

use super::dbus::{Nid, NotificationEvent};
use super::jobs::SyncList;
use super::listener::Listener;
use super::transmission::Transmission;
use crate::utils::{
    logger::Logger,
    macros::{async_rt::block_on_io, multithread::acquire_lock_panic},
};

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
    pub auto_close: bool,
    // The protocol specifies a timeout in i32. Since a negative timeout is impossible (except for
    // the special case of -1), u16 is used instead.
    pub default_timeout: u16,
}

// There is no guarantee that logs_buffer is synchronised with
// the logs_path file. This is not ACID compliant.
pub struct LogsManager {
    list: SyncList,
    listener_path: String,
    logs_path: String,
    logs_buffer: VecDeque<NotificationEvent>,
    logs_config: LogsConfig,
    dirty: bool,
    pub(super) interface: Option<Connection>,
    pub(super) notifyer: Option<Sender<()>>,
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

    pub fn new(
        list: SyncList,
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
                                "This will create a new file once a notification has been sent.",
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
            list,
            listener_path: listener_path.to_owned(),
            logs_path: logs_path.to_owned(),
            logs_buffer: buffer,
            logs_config,
            dirty: false,
            interface: None,
            notifyer: None,
        }
    }

    #[inline(always)]
    pub(super) fn copy_sync_list(&self) -> SyncList {
        Arc::clone(&self.list)
    }

    #[inline(always)]
    pub(super) fn iter<'a>(&'a self) -> impl Iterator<Item = &'a NotificationEvent> {
        self.logs_buffer.iter()
    }

    // SAFETY: This is marked as `unsafe` since it returns a mutable reference to a given
    // notification, but makes no guarantees as for whether `self.dirty` is actually called.
    //
    // Anyone that uses this function must ensure that proper safeguards are implemented to
    // disallow operations such as marking a notification as 'not closed' even if it was before
    // (in this example, modifying this state may cause undefined behaviour in other programs).
    #[inline(always)]
    pub(super) unsafe fn iter_mut<'a>(
        &'a mut self,
    ) -> impl Iterator<Item = &'a mut NotificationEvent> {
        self.logs_buffer.iter_mut()
    }

    #[inline]
    pub(super) fn find<'a>(&'a self, nid: Nid) -> Option<&'a NotificationEvent> {
        self.iter().find(|ne| ne.id() == nid)
    }

    // SAFETY: This is marked as `unsafe` since it returns a mutable reference to a given
    // notification, but makes no guarantees as for whether `self.dirty` is actually called.
    //
    // Anyone that uses this function must ensure that proper safeguards are implemented to
    // disallow operations such as marking a notification as 'not closed' even if it was before
    // (in this example, modifying this state may cause undefined behaviour in other programs).
    #[inline]
    #[allow(unused)]
    pub(super) unsafe fn find_mut<'a>(&'a mut self, nid: Nid) -> Option<&'a mut NotificationEvent> {
        unsafe { self.iter_mut().find(|ne| ne.id() == nid) }
    }

    /// Function that writes this `LogsManager` as 'dirty'.
    /// This means that, when the `SyncList` becomes empty, this `LogsManager`
    /// will write to the `logs_path` the `logs_buffer`. It doesn't make it
    /// ACID compliant, but if the logs are written, the next time this daemon
    /// is called, it will be up to date with all previous notification changes.
    #[inline(always)]
    pub(super) fn set_dirty(&mut self) {
        self.dirty = true;
    }

    pub(super) fn remove_notification(&mut self, nid: Nid) -> Option<NotificationEvent> {
        let pos = self.logs_buffer.iter().position(|val| val.id() == nid);
        if pos.is_none() {
            return None;
        }
        self.set_dirty();
        self.logs_buffer.swap_remove_back(pos.unwrap())
    }

    pub(super) fn append_notification(&mut self, notif: NotificationEvent) {
        if self.logs_buffer.len() == self.logs_config.max_logs as usize {
            self.logs_buffer.pop_back();
        }
        self.set_dirty();
        self.logs_buffer.push_front(notif);
    }

    pub(super) fn read_notification(&mut self, not: Nid) {
        // PERF: Since the notifications are appended in the front and most notifications are
        // marked as 'read' the moment they are sent, this will usually run with only 1 iteration.
        // Note though, that logs_buffer has no guarantee to be ordered, so the worst case scenario
        // is still O(n).
        self.logs_buffer
            .iter_mut()
            .find(|val| val.id() == not)
            .map(|not| {
                not.read = true;
            });
        self.set_dirty();
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

    #[inline(always)]
    pub fn logs_config<'a>(&'a self) -> &'a LogsConfig {
        &self.logs_config
    }

    pub fn map_timeout(&self, timeout: i32) -> Result<Option<u16>, &'static str> {
        match timeout {
            -1 => Ok(Some(self.logs_config.default_timeout)),
            0 => Ok(None),
            x @ 1..=i32::MAX => Ok(Some(x as u16)),
            _ => Err("Cannot have a negative timeout"),
        }
    }

    pub fn exec(&mut self) -> io::Result<ExecState> {
        let mut vd = acquire_lock_panic!(self.list.lock(), "LogsManager",);

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
        block_on_io!(async {
            let stream = UnixStream::connect(&self.listener_path).await?;
            Listener::write(&stream, &Transmission::from_fulfilled(&fj, jd.desc.pid)).await
        })?;

        Ok(if fj.result.is_err() {
            Logger::cdebug("(MANAGER THREAD): Error in last job execution.", None);
            ExecState::Error
        } else {
            ExecState::Executed
        })
    }
}
