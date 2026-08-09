use super::super::manager::LogsManager;
use super::super::transmission::{Payload, PayloadError};
use super::{Desc, FulfilledJob, FulfilledJobResultType, Job};

pub struct Watch;

impl Job for Watch {
    fn execute(self: Box<Self>, _: &Desc, _: &mut LogsManager) -> FulfilledJob {
        FulfilledJob::new(Ok(None), FulfilledJobResultType::Other)
    }

    fn canonical_name(&self) -> &'static str {
        "watch"
    }
}

impl Payload for Watch {
    fn from_str_static(data: &str) -> Result<Watch, PayloadError> {
        if data == "watch" {
            Ok(Watch)
        } else {
            Err(PayloadError::new(
                data.to_owned(),
                "Invalid payload".to_owned(),
                String::new(),
            ))
        }
    }

    fn to_string(&self) -> String {
        self.canonical_name().to_owned()
    }
}
