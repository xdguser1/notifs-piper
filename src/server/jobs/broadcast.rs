use super::super::dbus::NotificationEvent;
use super::super::manager::LogsManager;
use super::super::transmission::{Payload, PayloadError};
use super::{Desc, FulfilledJob, FulfilledJobResultType, Job};

pub struct Broadcast {
    event: NotificationEvent,
}

impl Broadcast {
    pub fn new(event: NotificationEvent) -> Broadcast {
        Broadcast { event }
    }
}

impl Job for Broadcast {
    fn execute(self: Box<Self>, _: &Desc, manager: &mut LogsManager) -> FulfilledJob {
        let broadcast = self.to_string();
        let replacement = self.event.get_replacement();
        if replacement != 0 {
            manager.remove_notification(replacement);
        }
        manager.append_notification(self.event);
        FulfilledJob::new(Ok(Some(broadcast)), FulfilledJobResultType::Notifications)
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
