use tokio::runtime::{Builder, LocalOptions};

use super::super::dbus::{Nid, NotificationsWrapperSignals};
use super::super::manager::LogsManager;
use super::super::transmission::{Payload, PayloadError};
use super::{Desc, EventType, FulfilledJob, FulfilledJobResultType, Job};

pub type NotificationClosedRepr = u32;

#[repr(u8)]
#[derive(Copy, Clone)]
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
            _ => unreachable!("Not implemented in specs."),
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
    fn execute(self: Box<Self>, _: &Desc, man: &mut LogsManager) -> FulfilledJob {
        let res = Builder::new_current_thread().build_local(LocalOptions::default());
        if res.is_err() {
            return FulfilledJob::new(
                Err(res.unwrap_err().to_string()),
                FulfilledJobResultType::Other,
            );
        }
        let res = res.unwrap().block_on(async {
            let Some(ref con) = man.interface else {
                return Err("Interface unset in manager. Function 'start_server' has a problem.");
            };
            con.object_server()
                .interface("/org/freedesktop/Notifications.")
                .await
                .map_err(|_| "Cannot connect to interface.")?
                .notification_closed(self.id, self.reason as NotificationClosedRepr)
                .await
                .map_err(|_| "Cannot emit 'notification_closed' signal.")?;
            Ok(None)
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
        let (id, reason) = data.split_once('#').ok_or(PayloadError::new(
            data.to_owned(),
            "'#' was not found in split.".to_owned(),
            String::new(),
        ))?;

        Ok(Close::new(
            id.parse::<Nid>().map_err(|err| {
                PayloadError::new(
                    id.to_owned(),
                    "Could not parse 'id' for 'Close' job.".to_owned(),
                    err.to_string(),
                )
            })?,
            reason
                .parse::<NotificationClosedRepr>()
                .map_err(|err| {
                    PayloadError::new(
                        reason.to_owned(),
                        "Could not parse 'reason' in 'Close' job.".to_owned(),
                        err.to_string(),
                    )
                })?
                .into(),
        ))
    }

    fn to_string(&self) -> String {
        format!("{}#{}", self.id, self.reason as NotificationClosedRepr)
    }
}
