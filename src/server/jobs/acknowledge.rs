use super::super::dbus::Nid;
use super::super::manager::LogsManager;
use super::super::transmission::{Payload, PayloadError};
use super::{Desc, FulfilledJob, FulfilledJobResultType, Job};

pub struct Acknowledge(pub Nid);

impl Job for Acknowledge {
    fn execute(self: Box<Self>, _: &Desc, manager: &mut LogsManager) -> FulfilledJob {
        manager.read_notification(self.0);
        FulfilledJob::new(Ok(None), FulfilledJobResultType::Other)
    }

    fn canonical_name(&self) -> &'static str {
        "close"
    }
}

impl Payload for Acknowledge {
    fn from_str_static(_: &str) -> Result<Acknowledge, PayloadError> {
        unimplemented!(
            "'Acknowledge' is only currently for internal use. This should not be called"
        );
    }

    fn to_string(&self) -> String {
        unimplemented!(
            "'Acknowledge' is only currently for internal use. This should not be called"
        );
    }
}
