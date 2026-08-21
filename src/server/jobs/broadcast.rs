use std::thread;
use std::time::{Duration, Instant};

use super::super::dbus::NotificationEvent;
use super::super::manager::LogsManager;
use super::super::transmission::{Payload, PayloadError};
use super::{Close, Desc, FulfilledJob, FulfilledJobResultType, Job, JobDesc, NotificationClosed};

pub struct Broadcast {
    event: NotificationEvent,
}

impl Broadcast {
    pub fn new(event: NotificationEvent) -> Broadcast {
        Broadcast { event }
    }
}

impl Job for Broadcast {
    fn execute(self: Box<Self>, _: &Desc, man: &mut LogsManager) -> FulfilledJob {
        let broadcast = self.to_string();
        let replacement = self.event.replacement();

        if replacement != 0 {
            man.remove_notification(replacement);
        }

        let config = man.logs_config();
        let timeout = self.event.timeout();
        // Silently fails if there is an invalid timeout.
        // So if x in i32::MIN..=-2 then x ~ 0 as far as timeout is concerned.
        if config.auto_close && timeout != 0 && timeout >= -1 {
            let wakeup = Instant::now()
                + Duration::from_millis(
                    man.map_timeout(timeout)
                    .unwrap() // timeout != 0 && timeout >= -1 => Ok
                    .unwrap() // timeout != 0 => Some
                    as u64,
                );

            let copy = man.copy_sync_list();

            let id = self.event.id();

            let snd = man.notifyer.clone();

            // WARNING: This may be out of date when it actually creates a new NotificationClosed
            // event (i.e. there was a replacement notification with a different timeout).
            // The easiest way to manage this is to double check in *Close* whether the
            // notification is actually expired.
            thread::spawn(move || {
                thread::sleep(wakeup - Instant::now());
                // Unwraps since, if the lock is poisoned, we have bigger problems than this
                // which are assumed to be managed elsewhere.
                copy.lock().unwrap().push_back(JobDesc::new(
                    Box::new(Close::new(id, NotificationClosed::Expired)),
                    Desc::new(0, 0),
                ));

                snd.map(|val| {
                    // Ignores the error for the same reason as above.
                    let _ = val.send(());
                });
            });
        }

        // No race conditions with the previous thread since manager executes
        // one job at a time. So a notification cannot be closed before it is added.
        man.append_notification(self.event);

        FulfilledJob::new(Ok(Some(broadcast)), FulfilledJobResultType::Results)
    }

    fn canonical_name(&self) -> &'static str {
        "broadcast"
    }
}

impl Payload for Broadcast {
    fn from_str_static(data: &str) -> Result<Broadcast, PayloadError> {
        serde_json::from_str::<NotificationEvent>(data)
            .map(|val| Broadcast::new(val))
            .map_err(|err| {
                PayloadError::new(
                    data.to_owned(),
                    "Error while parsing 'event' for 'Broadcast'".to_owned(),
                    err.to_string(),
                )
            })
    }

    fn to_string(&self) -> String {
        serde_json::to_string(&self.event).unwrap()
    }
}
