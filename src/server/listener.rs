use std::collections::HashMap;
use std::io::{self, Error, ErrorKind};
use std::sync::{
    Arc, Mutex,
    mpsc::{Sender, channel},
};

use tokio::net::{UnixListener, UnixStream};
use tokio::task::JoinSet;

use crate::utils::logger::Logger;

use super::dbus::NotificationEvent;
use super::jobs::{Acknowledge, Desc, Flags, FlagsRepr, JobDesc, SyncList};
use super::transmission::{Payload, Transmission, TransmissionType};

pub struct Listener {
    active_processes: Arc<Mutex<HashMap<u32, UnixStream>>>,
    listening_processes: Arc<tokio::sync::Mutex<HashMap<u32, (UnixStream, FlagsRepr)>>>,
    // `String` is not strictly required, since a lifetime could do the job, but better
    // for future features, if any require a owned path.
    listening_path: String,
    sync_list: SyncList,
    notify: Sender<()>,
}

impl Listener {
    async fn read(us: &UnixStream) -> io::Result<Transmission> {
        let mut tdata = String::new();
        let mut buf: [u8; 4096] = [0; 4096];

        loop {
            us.readable().await?;

            match us.try_read(&mut buf) {
                Ok(n) => {
                    if n == 0 {
                        break;
                    }

                    tdata += std::str::from_utf8(&buf)
                        .map_err(|err| Error::new(ErrorKind::InvalidInput, err))?;
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

    async fn write(us: &UnixStream, tr: &Transmission) -> io::Result<()> {
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

            let lp = Arc::clone(&self.listening_processes);
            let ap = Arc::clone(&self.active_processes);
            let sl = Arc::clone(&self.sync_list);

            let sn = self.notify.clone();
            let pn = panic.clone();

            active.spawn(async move {
                let trans = Listener::read(&us).await;

                if let Err(err) = &trans {
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
                        if let Err(_) = $id.send(()) {
                            pn.send(()).unwrap();
                            return;
                        }
                    };
                }

                match trans.typ {
                    TT::Error => {
                        Listener::send_error(&us, "Current architecture wants the client to manage errors.".to_owned()).await;
                    },
                    TT::Incoming(pid) => {
                        match JobDesc::from_str_static(trans.data.as_str()) {
                            Ok(jobd) => {
                                if jobd.cmd.canonical_name() == "watch" {
                                    lp.lock().await.insert(pid, (us, jobd.desc.flags));
                                    return;
                                }

                                acquire_lock!(ap, "active_processes", lock, lock.insert(pid, us));
                                acquire_lock!(sl, "sync_list", lock, lock.push_back(jobd));
                            },
                            Err(err) => {
                                Listener::send_error(&us, err.error).await;
                                return;
                            },
                        }

                        notify_manager!(sn);
                    },
                    TT::Outgoing(pid) => {
                        if pid == 0 {
                            let mut lock = lp.lock().await;
                            let mut old = Vec::with_capacity(lock.len());
                            let mut read = false;
                            for (unx, flags) in lock.values() {
                                if let Err(_) = Listener::write(
                                    unx,
                                    &Transmission::new(TransmissionType::Outgoing(pid), trans.data.clone())
                                ).await {
                                    read |= Flags::SILENT.is(*flags);
                                    old.push(pid);
                                }
                            }
                            old.iter().for_each(|val| { lock.remove(val); });

                            if read && let Ok(nev) = serde_json::from_str(&trans.data).and_then(|val| serde_json::from_value::<NotificationEvent>(val)) {
                                acquire_lock!(sl, "sync_list", lock, lock.push_back(JobDesc::new(Box::new(Acknowledge(nev.get_id())), Desc::new(0, 0))));

                                notify_manager!(sn);
                            }
                           return;
                        }

                        let us = acquire_lock!(
                            ap,
                            "active_processes",
                            lock,
                            lock.remove(&pid)
                        );

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
