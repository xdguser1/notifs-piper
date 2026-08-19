use super::super::dbus::Nid;
use super::super::transmission::{Payload, PayloadError};
use super::{Desc, FulfilledJob, FulfilledJobResultType, Job, LogsManager};
use crate::utils::logger::Logger;

pub struct Query(pub Nid);

impl Job for Query {
    fn execute(self: Box<Self>, _: &Desc, man: &mut LogsManager) -> FulfilledJob {
        Logger::cdebug(
            "(MANAGER THREAD): Querying to see if notification is closed.",
            None,
        );

        FulfilledJob::new(
            Ok(Some(format!(
                "{}",
                man.find(self.0).map(|val| val.is_closed()).unwrap_or(false)
            ))),
            FulfilledJobResultType::Results,
        )
    }

    fn canonical_name(&self) -> &'static str {
        "query"
    }
}

impl Payload for Query {
    fn from_str_static(data: &str) -> Result<Query, PayloadError> {
        Ok(Query(data.parse::<Nid>().map_err(|err| {
            PayloadError::new(
                data.to_owned(),
                "Could not parse 'Nid' for 'Query'.".to_owned(),
                err.to_string(),
            )
        })?))
    }

    fn to_string(&self) -> String {
        self.0.to_string()
    }
}
