use super::super::manager::{LogsManager, Watcher};
use super::super::transmission::{Payload, PayloadError};
use super::{Desc, FulfilledJob, Job};

pub struct Watch;

impl Job for Watch {
    fn execute(self: Box<Self>, desc: &Desc, manager: &mut LogsManager) -> FulfilledJob {

        manager
            .active_processes
            .insert(Watcher::new(desc.pid, desc.flags));
        FulfilledJob(Ok(None))
    }

    fn canonical_name(&self) -> &'static str {
        "watch"
    }
}

impl Payload for Watch {
    fn from_str_static(data: &str) -> Result<Watch, PayloadError> {
        if data.is_empty() {
            Ok(Watch)
        } else {
            Err(PayloadError::new(
                data.to_owned(),
                "'Watch' received arguments".to_owned(),
                String::new(),
            ))
        }
    }

    fn to_string(&self) -> String {
        String::new()
    }
}
