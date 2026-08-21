use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use super::dbus::Nid;
use super::manager::LogsManager;
use super::transmission::{Payload, PayloadError};
use crate::utils::macros::parse::{parse, split_once};

pub(super) use self::acknowledge::*;
pub use self::broadcast::*;
pub use self::close::*;
pub use self::query::*;
pub use self::read::*;
pub use self::signals::*;
pub use self::watch::*;

mod acknowledge;
mod broadcast;
mod close;
mod query;
mod read;
mod signals;
mod watch;

pub type FlagsRepr = u8;
pub type Pid = u32;
pub type JobList = VecDeque<JobDesc>;
pub type SyncList = Arc<Mutex<JobList>>;

macro_rules! from_static_cast {
    ($struct_name:ident, $data_str:expr) => {
        Box::new($struct_name::from_str_static($data_str)?) as Box<dyn Job>
    };
}

pub trait Job: Payload + Send {
    fn execute(self: Box<Self>, desc: &Desc, manager: &mut LogsManager) -> FulfilledJob;
    // Forces every job to have a canonical name and respect the format `desc##name##args`
    // even though this could be simply added to the `to_string` method
    fn canonical_name(&self) -> &'static str;
}

fn from_str_static_dyn_payload(data: &str) -> Result<Box<dyn Job>, PayloadError> {
    let (typ, data) = split_once!(data, "##")?;

    Ok(match typ {
        "broadcast" => from_static_cast!(Broadcast, data),
        "close" => from_static_cast!(Close, data),
        "read" => from_static_cast!(Read, data),
        "watch" => from_static_cast!(Watch, data),
        "action" => from_static_cast!(ActionInvoked, data),
        "activation" => from_static_cast!(ActivationToken, data),
        "query" => from_static_cast!(Query, data),
        _ => {
            unreachable!("Someone forgot to put their canonical name here.");
        }
    })
}

pub enum EventType {
    Close(Nid),
    ActionInvoked(Nid, String),
    ActivationToken(Nid, String),
}

impl Payload for EventType {
    fn from_str_static(data: &str) -> Result<EventType, PayloadError> {
        let (typ, rest) = split_once!(data, '#')?;

        match typ {
            "cls" => Ok(EventType::Close(parse!(rest, Nid, "EventType")?)),
            "act" => {
                let (nid, act) = split_once!(rest, '#')?;
                Ok(EventType::ActionInvoked(
                    parse!(nid, Nid, "EventType")?,
                    act.to_owned(),
                ))
            }
            "acv" => {
                let (nid, acv) = split_once!(rest, '#')?;
                Ok(EventType::ActivationToken(
                    parse!(nid, Nid, "EventType")?,
                    acv.to_owned(),
                ))
            }
            _ => Err(PayloadError::new(
                typ.to_owned(),
                "Bad type for 'EventType'.".to_owned(),
                String::new(),
            )),
        }
    }

    fn to_string(&self) -> String {
        match self {
            EventType::Close(nid) => format!("cls#{}", nid),
            EventType::ActionInvoked(nid, action_key) => format!("act#{}#{}", nid, action_key),
            EventType::ActivationToken(nid, activation_token) => {
                format!("acv#{}#{}", nid, activation_token)
            }
        }
    }
}

pub enum FulfilledJobResultType {
    Results,
    Event(EventType),
    Other,
}

pub struct FulfilledJob {
    pub result: Result<Option<String>, String>,
    pub typ: FulfilledJobResultType,
}

impl FulfilledJob {
    pub fn new(
        result: Result<Option<String>, String>,
        typ: FulfilledJobResultType,
    ) -> FulfilledJob {
        FulfilledJob { result, typ }
    }
}

impl Payload for FulfilledJob {
    fn from_str_static(data: &str) -> Result<FulfilledJob, PayloadError> {
        if data.is_empty() {
            return Ok(FulfilledJob::new(Ok(None), FulfilledJobResultType::Other));
        }

        let (typ, data) = split_once!(data, '#')?;

        match typ {
            "err" => Ok(FulfilledJob::new(
                Err(data.to_owned()),
                FulfilledJobResultType::Other,
            )),
            "res" => Ok(FulfilledJob::new(
                Ok(Some(data.to_owned())),
                FulfilledJobResultType::Results,
            )),
            "ev" => Ok(FulfilledJob::new(
                Ok(None),
                FulfilledJobResultType::Event(EventType::from_str_static(data)?),
            )),
            "oth" => Ok(FulfilledJob::new(
                Ok(Some(data.to_owned())),
                FulfilledJobResultType::Other,
            )),
            _ => Err(PayloadError::new(
                typ.to_owned(),
                "Invalid type for 'FulfilledJob'.".to_owned(),
                String::new(),
            )),
        }
    }

    fn to_string(&self) -> String {
        if let FulfilledJobResultType::Event(ev) = &self.typ {
            // Anything that is in "result" will be ignored.
            return format!("ev#{}", ev.to_string());
        } else if let FulfilledJobResultType::Other = &self.typ {
            return format!("oth#{:?}", &self.result);
        }

        match &self.result {
            Err(err) => format!("err#{}", err),
            Ok(opt) if opt.as_ref().is_some_and(|v| !v.is_empty()) => {
                format!("res#{}", opt.as_ref().clone().unwrap())
            }
            _ => "".to_owned(),
        }
    }
}

#[repr(u8)]
#[derive(Copy, Clone)]
pub enum Flags {
    NONE = 0x00,
    SILENT = 0x01,
    FORCE = 0x02,
}

impl Flags {
    pub const fn is(&self, int: FlagsRepr) -> bool {
        (*self as FlagsRepr) & int != 0
    }

    pub const fn join(int: FlagsRepr, other: Flags) -> FlagsRepr {
        int | (other as FlagsRepr)
    }
}

pub struct Desc {
    pub pid: Pid,
    pub flags: FlagsRepr,
}

impl Desc {
    pub const fn new(pid: Pid, flags: FlagsRepr) -> Desc {
        Desc { pid, flags }
    }
}

impl Payload for Desc {
    fn from_str_static(data: &str) -> Result<Desc, PayloadError> {
        let (first, second) = split_once!(data, '#')?;

        Ok(Desc::new(
            parse!(first, Pid, "Desc")?,
            parse!(second, FlagsRepr, "Desc")?,
        ))
    }

    fn to_string(&self) -> String {
        format!("{}#{}", self.pid, self.flags)
    }
}

pub struct JobDesc {
    pub cmd: Box<dyn Job>,
    pub desc: Desc,
}

impl Payload for JobDesc {
    fn from_str_static(data: &str) -> Result<JobDesc, PayloadError> {
        let (desc, job) = split_once!(data, "##")?;

        Ok(JobDesc::new(
            from_str_static_dyn_payload(job)?,
            Desc::from_str_static(desc)?,
        ))
    }

    fn to_string(&self) -> String {
        self.desc.to_string()
            + "##"
            + self.cmd.canonical_name()
            + "##"
            + self.cmd.to_string().as_str()
    }
}

impl JobDesc {
    pub fn new(cmd: Box<dyn Job>, desc: Desc) -> JobDesc {
        JobDesc { cmd, desc }
    }
}
