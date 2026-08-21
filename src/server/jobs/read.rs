use super::super::manager::{LogCountType, LogsManager};
use super::super::transmission::{Payload, PayloadError};
use super::{Desc, Flags, FulfilledJob, FulfilledJobResultType, Job};
use crate::utils::macros::parse::{parse, split_once};

pub struct Read {
    start: LogCountType,
    end: LogCountType,
}

impl Read {
    pub fn new(start: LogCountType, end: LogCountType) -> Read {
        Read { start, end }
    }
}

impl Job for Read {
    fn execute(self: Box<Self>, desc: &Desc, manager: &mut LogsManager) -> FulfilledJob {
        FulfilledJob::new(
            Ok(Some(
                serde_json::to_string(manager.read_logs(
                    self.start,
                    self.end,
                    !Flags::SILENT.is(desc.flags),
                ))
                .unwrap(),
            )),
            FulfilledJobResultType::Results,
        )
    }

    fn canonical_name(&self) -> &'static str {
        "read"
    }
}

impl Payload for Read {
    fn from_str_static(data: &str) -> Result<Read, PayloadError> {
        let (start, end) = split_once!(data, '#')?;

        Ok(Read::new(
            parse!(start, LogCountType, "Read")?,
            parse!(end, LogCountType, "Read")?,
        ))
    }

    fn to_string(&self) -> String {
        format!("{}#{}", self.start, self.end)
    }
}
