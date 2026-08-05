use super::super::manager::{LogCountType, LogsManager};
use super::super::transmission::{Payload, PayloadError};
use super::{Desc, Flags, FulfilledJob, FulfilledJobResultType, Job};

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
                    Flags::SILENT.is(desc.flags),
                ))
                .unwrap(),
            )),
            FulfilledJobResultType::Notifications,
        )
    }

    fn canonical_name(&self) -> &'static str {
        "job"
    }
}

impl Payload for Read {
    fn from_str_static(data: &str) -> Result<Read, PayloadError> {
        let (start, end) = data.split_once('#').ok_or(PayloadError::new(
            data.to_owned(),
            "'#' not found in arguments for 'Read'".to_owned(),
            String::new(),
        ))?;

        Ok(Read::new(
            start.parse::<LogCountType>().map_err(|err| {
                PayloadError::new(
                    start.to_owned(),
                    "Error while parsing 'LogCountType' in 'Read'".to_owned(),
                    err.to_string(),
                )
            })?,
            end.parse::<LogCountType>().map_err(|err| {
                PayloadError::new(
                    end.to_owned(),
                    "Error while parsing 'LogCountType' in 'Read'".to_owned(),
                    err.to_string(),
                )
            })?,
        ))
    }

    fn to_string(&self) -> String {
        format!("{}#{}", self.start, self.end)
    }
}
