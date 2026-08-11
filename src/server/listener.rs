use std::collections::HashMap;
use std::io::{self, Error, ErrorKind};
use std::sync::{
    Arc, Mutex,
    mpsc::{Sender, channel},
};

use tokio::net::{UnixListener, UnixStream};
use tokio::task::JoinSet;

use crate::server::jobs::{FulfilledJob, FulfilledJobResultType};
use crate::utils::logger::Logger;

use super::dbus::NotificationEvent;
use super::jobs::{Acknowledge, Desc, Flags, FlagsRepr, JobDesc, Pid, SyncList};
use super::transmission::{Payload, Transmission, TransmissionType};

pub struct Listener {
    active_processes: Arc<Mutex<HashMap<Pid, UnixStream>>>,
    listening_processes: Arc<tokio::sync::Mutex<HashMap<Pid, (UnixStream, FlagsRepr)>>>,
    // `String` is not strictly required, since a lifetime could do the job, but better
    // for future features, if any require a owned path.
    listening_path: String,
    sync_list: SyncList,
    notify: Sender<()>,
}

impl Listener {
    pub async fn read(us: &UnixStream) -> io::Result<Transmission> {
        const BUF_SIZE: usize = 4096;

        let mut tdata = String::new();
        let mut buf: [u8; BUF_SIZE] = [0; BUF_SIZE];

        loop {
            us.readable().await?;

            match us.try_read(&mut buf) {
                Ok(n) => {
                    tdata += std::str::from_utf8(&buf[0..n])
                        .map_err(|err| Error::new(ErrorKind::InvalidInput, err))?;
                    if n < BUF_SIZE {
                        break;
                    }
                }
                Err(err) if err.kind() == ErrorKind::WouldBlock => {
                    continue;
                }
                Err(err) => {
                    return Err(err);
                }
            }
        }

        Ok(<Transmission as Payload>::from_str_static(tdata.as_str())
            .map_err(|err| Error::new(ErrorKind::InvalidData, err.data))?)
    }

    pub async fn write(us: &UnixStream, tr: &Transmission) -> io::Result<()> {
        let data = tr.to_string();
        let mut buf = data.as_bytes();

        loop {
            us.writable().await?;

            match us.try_write(&buf) {
                Ok(n) => {
                    if buf.is_empty() {
                        break;
                    }
                    buf = &buf[n..];
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

    async fn send_error(us: &UnixStream, err: String) {
        if let Err(errl) =
            Listener::write(&us, &Transmission::new(TransmissionType::Error, err)).await
        {
            Logger::error(
                format!(
                    "Could not send back error to client.\nError type: {}",
                    errl.to_string(),
                )
                .as_str(),
            );
        }
    }

    pub fn new(listening_path: &str, sync_list: &SyncList, notify: Sender<()>) -> Listener {
        Listener {
            active_processes: Arc::new(Mutex::new(HashMap::new())),
            listening_processes: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            listening_path: listening_path.to_owned(),
            sync_list: Arc::clone(sync_list),
            notify,
        }
    }

    pub async fn listen(&self) -> io::Result<!> {
        use TransmissionType as TT;

        let listener = UnixListener::bind(&self.listening_path)?;
        let mut active = JoinSet::new();
        let (panic, abort) = channel::<()>();

        loop {
            if abort.try_recv().is_ok() {
                return Err(io::Error::new(ErrorKind::Other, "Details in stdout."));
            }

            let us = listener.accept().await?.0;
            Logger::cdebug("(LISTENING THREAD): Accepting transmission.", None);

            let lp = Arc::clone(&self.listening_processes);
            let ap = Arc::clone(&self.active_processes);
            let sl = Arc::clone(&self.sync_list);

            let sn = self.notify.clone();
            let pn = panic.clone();

            active.spawn(async move {
                let trans = Listener::read(&us).await;

                if let Err(err) = &trans {
                    Logger::cdebug("(LISTENING THREAD): Error in transmission.", None);
                    Listener::send_error(&us, err.to_string()).await;
                    return;
                }

                let trans = trans.unwrap();

                macro_rules! acquire_lock {
                    ($id:ident, $lit:literal, $lock:ident, $($ts:tt)*) => {
                        match (*$id).lock() {
                            Ok(mut $lock) => {
                                $($ts)*
                            },
                            Err(poison) => {
                                Logger::error(
                                    format!(
                                        concat!(
                                            "!!FATAL ERROR!! A thread panicked while holding the '{}' lock. Exiting.\n",
                                            "Error type: {}",
                                        ),
                                        $lit,
                                        poison.to_string(),
                                    ).as_str()
                                );
                                pn.send(()).unwrap();
                                return;
                            },
                        }
                    };
                }

                macro_rules! notify_manager {
                    ($id:ident) => {
                        Logger::cdebug("(LISTENING THREAD): Notifying manager of new job.", None);
                        if let Err(_) = $id.send(()) {
                            Logger::cdebug("(LISTENING THREAD): Manager down. FATAL ERROR.", None);
                            pn.send(()).unwrap();
                            return;
                        }
                    };
                }

                match trans.typ {
                    TT::Error => {
                        Logger::cdebug("(LISTENING THREAD): Received error from client. THIS SHOULD NOT HAPPEN.", None);
                        Listener::send_error(&us, "Current architecture wants the client to manage errors.".to_owned()).await;
                    },
                    TT::Incoming(pid) => {
                        match JobDesc::from_str_static(trans.data.as_str()) {
                            Ok(jobd) => {
                                if jobd.cmd.canonical_name() == "watch" {
                                    Logger::cdebug("(LISTENING THREAD): Received watch request.", None);
                                    lp.lock().await.insert(pid, (us, jobd.desc.flags));
                                    return;
                                }

                                Logger::cdebug("(LISTENING THREAD): Pushing new job to list.", None);
                                acquire_lock!(ap, "active_processes", lock, lock.insert(pid, us));
                                acquire_lock!(sl, "sync_list", lock, lock.push_back(jobd));
                            },
                            Err(err) => {
                                Logger::cdebug("(LISTENING THREAD): No matching job found when parsing transmission.", None);
                                Listener::send_error(&us, err.error).await;
                                return;
                            },
                        }

                        notify_manager!(sn);
                    },
                    TT::Outgoing(pid) => {
                        if pid == 0 {
                            Logger::cdebug("(LISTENING THREAD): Received broadcast.", None);

                            let mut lock = lp.lock().await;
                            let mut old = Vec::with_capacity(lock.len());
                            let mut read = false;
                            let prevl = lock.len();

                            Logger::cdebug(format!("(LISTENING THREAD): Current broadcasting list: {} processes.", prevl).as_str(), None);
                            for (pidx, (unx, flags)) in lock.iter() {
                                if let Err(_) = Listener::write(
                                    unx,
                                    &Transmission::new(TransmissionType::Outgoing(pid), trans.data.clone())
                                ).await {
                                    old.push(*pidx);
                                } else {
                                    read |= !Flags::SILENT.is(*flags);
                                }
                            }
                            old.iter().for_each(|val| { lock.remove(val); });
                            let nextl = old.len();
                            Logger::cdebug(format!("(LISTENING THREAD): Current broadcasting list: {} - {} = {} processes left.", prevl, nextl, prevl - nextl).as_str(), None);

                            if read && let Ok(not) = FulfilledJob::from_str_static(&trans.data) && let FulfilledJobResultType::Notifications = not.typ {
                                Logger::cdebug("(LISTENING THREAD): Modifying 'read' state of new notification to 'true'.", None);
                                acquire_lock!(sl, "sync_list", lock, lock.push_back(JobDesc::new(Box::new(Acknowledge(serde_json::from_str::<NotificationEvent>(not.result.unwrap().unwrap().as_str()).unwrap().get_id())), Desc::new(0, 0))));

                                notify_manager!(sn);
                            }
                           return;
                        }

                        Logger::cdebug("(LISTENING THREAD): Removing holding connection (results received).", None);
                        let (us, len) = acquire_lock!(
                            ap,
                            "active_processes",
                            lock,
                            (lock.remove(&pid), lock.len())
                        );
                        Logger::cdebug(format!("(LISTENING THREAD): Process removed from waiting list: {} processes left.", len).as_str(), None);

                        Logger::cdebug("(LISTENING THREAD): Received executed job. Transmitting results.", None);
                        if let Err(err) = Listener::write(&us.unwrap(), &trans).await {
                            Logger::error(
                                format!(
                                    concat!(
                                        "Could not send data back to process {}.\n",
                                        "Error type: {}",
                                    ),
                                    pid,
                                    err.to_string()
                                ).as_str()
                            );
                        }
                    },
                }
            });
        }
    }
}
