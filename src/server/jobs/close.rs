use std::time::{SystemTime, UNIX_EPOCH};

use tokio::runtime::{Builder, LocalOptions};

use crate::utils::{
    logger::Logger,
    macros::parse::{parse, split_once},
};
use super::super::dbus::{Nid, NotificationsWrapperSignals};
use super::super::manager::LogsManager;
use super::super::transmission::{Payload, PayloadError};
use super::{Desc, EventType, Flags, FulfilledJob, FulfilledJobResultType, Job};

pub type NotificationClosedRepr = u32;

#[repr(u8)]
#[derive(Copy, Clone, PartialEq)]
pub enum NotificationClosed {
    Expired = 1,
    Dismissed = 2,
    CallCloseNotification = 3,
    Undefined = 4,
}

impl From<NotificationClosedRepr> for NotificationClosed {
    fn from(value: NotificationClosedRepr) -> NotificationClosed {
        use NotificationClosed as NC;

        match value {
            1 => NC::Expired,
            2 => NC::Dismissed,
            3 => NC::CallCloseNotification,
            4 => NC::Undefined,
            _ => unreachable!("Not implemented in the specs."),
        }
    }
}

pub struct Close {
    id: Nid,
    reason: NotificationClosed,
}

impl Close {
    pub fn new(id: Nid, reason: NotificationClosed) -> Close {
        Close { id, reason }
    }
}

impl Job for Close {
    fn execute(self: Box<Self>, desc: &Desc, man: &mut LogsManager) -> FulfilledJob {
        Logger::cdebug("(MANAGER THREAD): Received closure request.", None);

        let pos = man.iter().position(|val| val.id() == self.id);
        let ne = pos.clone().and_then(|val| man.iter().nth(val));

        // If it is closed or does not exist whilst the --force option is down: silently fail.
        if ne.as_ref().is_none_or(|ne| ne.closed()) && !Flags::FORCE.is(desc.flags) {
            // FulfilledJobResultType::Other and not FulfilledJobResultType::Results since it
            // technically fails, even though it does not matter since it is never communicated
            // with the client through Listener.
            return FulfilledJob::new(Ok(None), FulfilledJobResultType::Other);
        }

        let ne = ne.unwrap();

        let timeout = ne.timeout();
        let time = ne.time();
        if self.reason == NotificationClosed::Expired
            && man.logs_config().auto_close
            && (timeout == 0
                || man
                    .map_timeout(timeout)
                    .and_then(|val| {
                        if SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap()
                            .as_millis()
                            < val.unwrap() as u128 + time
                        {
                            return Err("");
                        }
                        Ok(Option::<u16>::None)
                    })
                    .is_err())
        {
            // This job is silently dropped, since the notification is not yet timed out.
            return FulfilledJob::new(Ok(None), FulfilledJobResultType::Other);
        }

        Logger::cdebug("(MANAGER THREAD): Closing notification.", None);

        // SAFETY: man.set_dirty() called just after. This is fine and required because of
        // the borrow safety between ne and man.
        let ne = unsafe { man.iter_mut().nth(pos.unwrap()).unwrap() };

        ne.set_closed();
        man.set_dirty();

        let res = Builder::new_current_thread().build_local(LocalOptions::default());
        if res.is_err() {
            return FulfilledJob::new(
                Err(res.unwrap_err().to_string()),
                FulfilledJobResultType::Other,
            );
        }

        let res = res.unwrap().block_on(async {
            if let Some(ref con) = man.interface {
                con.object_server()
                    .interface("/org/freedesktop/Notifications")
                    .await
                    .map_err(|_| "Cannot connect to interface.")?
                    .notification_closed(self.id, self.reason as NotificationClosedRepr)
                    .await
                    .map_err(|_| "Cannot emit 'notification_closed' signal.")?;

                Ok(Some("".to_owned()))
            } else {
                Err("Interface unset in manager. Function 'start_server' has a problem.")
            }
        });

        FulfilledJob::new(
            res.map_err(|stg| stg.to_owned()),
            FulfilledJobResultType::Event(EventType::Close(self.id)),
        )
    }

    fn canonical_name(&self) -> &'static str {
        "close"
    }
}

impl Payload for Close {
    fn from_str_static(data: &str) -> Result<Close, PayloadError> {
        let (id, reason) = split_once!(data, '#')?;

        Ok(Close::new(
            parse!(id, Nid, "Close")?,
            parse!(reason, NotificationClosedRepr, "Close")?.into(),
        ))
    }

    fn to_string(&self) -> String {
        format!("{}#{}", self.id, self.reason as NotificationClosedRepr)
    }
}
