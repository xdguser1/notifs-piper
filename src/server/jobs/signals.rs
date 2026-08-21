use tokio::runtime::{Builder, LocalOptions};

use crate::utils::{
    logger::Logger,
    macros::parse::{parse, split_once},
};
use super::super::dbus::{Nid, NotificationsWrapperSignals};
use super::super::transmission::{Payload, PayloadError};
use super::{Desc, EventType, Flags, FulfilledJob, FulfilledJobResultType, Job, LogsManager};

macro_rules! derive_signal {
    ($name:ident, $canonical:literal, $data_name:ident, $signal:ident) => {
        pub struct $name {
            id: Nid,
            $data_name: String,
        }

        impl $name {
            pub const fn new(id: Nid, $data_name: String) -> $name {
                $name { id, $data_name }
            }
        }

        impl Job for $name {
            fn execute(self: Box<Self>, desc: &Desc, man: &mut LogsManager) -> FulfilledJob {
                if man.find(self.id).is_none_or(|ne| ne.closed()) && !Flags::FORCE.is(desc.flags) {
                    return FulfilledJob::new(Ok(None), FulfilledJobResultType::Other);
                }

                Logger::cdebug("(MANAGER THREAD): Sending special signal.", None);

                let run = Builder::new_current_thread().build_local(LocalOptions::default());

                if run.is_err() {
                    return FulfilledJob::new(
                        Err(run.unwrap_err().to_string()),
                        FulfilledJobResultType::Other,
                    );
                }

                let res = run.unwrap().block_on(async {
                    if let Some(ref con) = man.interface {
                        con.object_server()
                            .interface("/org/freedesktop/Notifications")
                            .await
                            .map_err(|_| "Cannot connect to interface.")?
                            .$signal(self.id, self.$data_name.as_str())
                            .await
                            .map_err(|_| "Cannot emit signal.")?;
                        Ok(Some("".to_owned()))
                    } else {
                        Err("Interface unset in manager. Function 'start_server' has a problem.")
                    }
                });

                FulfilledJob::new(
                    res.map_err(|stg| stg.to_owned()),
                    FulfilledJobResultType::Event(EventType::$name(self.id, self.$data_name)),
                )
            }

            fn canonical_name(&self) -> &'static str {
                $canonical
            }
        }

        impl Payload for $name {
            fn from_str_static(data: &str) -> Result<$name, PayloadError> {
                let (id, $data_name) = split_once!(data, '#')?;

                Ok($name::new(parse!(id, Nid, "Signal")?, $data_name.to_owned()))
            }

            fn to_string(&self) -> String {
                format!("{}#{}", self.id, self.$data_name)
            }
        }
    };
}

derive_signal! {
    ActionInvoked,
    "action",
    action_key,
    action_invoked
}

derive_signal! {
    ActivationToken,
    "activation",
    activation_token,
    activation_token
}
