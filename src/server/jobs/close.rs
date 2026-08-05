use super::super::dbus::Nid;
use super::super::manager::LogsManager;
use super::super::transmission::{Payload, PayloadError};
use super::{Desc, EventType, FulfilledJob, FulfilledJobResultType, Job};

pub struct Close(pub Nid);

impl Job for Close {
    fn execute(self: Box<Self>, _: &Desc, _: &mut LogsManager) -> FulfilledJob {
        FulfilledJob::new(
            Ok(None),
            FulfilledJobResultType::Event(EventType::Close(self.0)),
        )
    }

    fn canonical_name(&self) -> &'static str {
        "close"
    }
}

impl Payload for Close {
    fn from_str_static(data: &str) -> Result<Close, PayloadError> {
        Ok(Close(data.parse::<Nid>().map_err(|err| {
            PayloadError::new(
                data.to_owned(),
                "Could not parse 'nid' for 'Close'".to_owned(),
                err.to_string(),
            )
        })?))
    }

    fn to_string(&self) -> String {
        self.0.to_string()
    }
}
