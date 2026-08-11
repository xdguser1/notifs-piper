use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use super::dbus::Nid;
use super::manager::LogsManager;
use super::transmission::{Payload, PayloadError};

pub(super) use self::acknowledge::*;
pub use self::broadcast::*;
pub use self::close::*;
pub use self::read::*;
pub use self::watch::*;

mod acknowledge;
mod broadcast;
mod close;
mod read;
mod watch;

pub type FlagsRepr = u8;
pub type Pid = u32;
pub type JobList = VecDeque<JobDesc>;
pub type SyncList = Arc<Mutex<JobList>>;

pub trait Job: Payload + Send {
    fn execute(self: Box<Self>, desc: &Desc, manager: &mut LogsManager) -> FulfilledJob;
    // Forces every job to have a canonical name and respect the format `desc##name##args`
    // even though this could be simply added to the `to_string` method
    fn canonical_name(&self) -> &'static str;
}

fn from_str_static_dyn_payload(data: &str) -> Result<Box<dyn Job>, PayloadError> {
    let (typ, data) = data.split_once("##").ok_or(PayloadError::new(
        data.to_owned(),
        "'##' was not found in split.".to_owned(),
        String::new(),
    ))?;

    macro_rules! branch {
        ($idn:ident) => {
            Box::new($idn::from_str_static(data)?) as Box<dyn Job>
        };
    }

    Ok(match typ {
        "broadcast" => branch!(Broadcast),
        "read" => branch!(Read),
        "watch" => branch!(Watch),
        _ => {
            unreachable!("Someone forgot to put their canonical name here.");
        }
    })
}

pub enum EventType {
    Close(Nid),
}

impl Payload for EventType {
    fn from_str_static(data: &str) -> Result<EventType, PayloadError> {
        let (typ, rest) = data.split_once('#').ok_or(PayloadError::new(
            data.to_owned(),
            "'#' was not found in split.".to_owned(),
            String::new(),
        ))?;

        match typ {
            "cls" => Ok(EventType::Close(rest.parse::<Nid>().map_err(|err| {
                PayloadError::new(
                    typ.to_owned(),
                    "Could not parse 'nid' for 'EventType' type 'Close'".to_owned(),
                    err.to_string(),
                )
            })?)),
            _ => Err(PayloadError::new(
                typ.to_owned(),
                "Bad type for 'EventType'".to_owned(),
                String::new(),
            )),
        }
    }

    fn to_string(&self) -> String {
        match *self {
            EventType::Close(nid) => format!("cls#{}", nid),
        }
    }
}

pub enum FulfilledJobResultType {
    Notifications,
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

        let (typ, data) = data.split_once('#').ok_or(PayloadError::new(
            data.to_owned(),
            "'#' was not found in split.".to_owned(),
            String::new(),
        ))?;

        match typ {
            "err" => Ok(FulfilledJob::new(
                Err(data.to_owned()),
                FulfilledJobResultType::Other,
            )),
            "not" => Ok(FulfilledJob::new(
                Ok(Some(data.to_owned())),
                FulfilledJobResultType::Notifications,
            )),
            "ev" => Ok(FulfilledJob::new(
                Ok(None),
                FulfilledJobResultType::Event(EventType::from_str_static(data)?),
            )),
            _ => Err(PayloadError::new(
                typ.to_owned(),
                "Invalid type for 'FulfilledJob'".to_owned(),
                String::new(),
            )),
        }
    }

    fn to_string(&self) -> String {
        if let FulfilledJobResultType::Event(ev) = &self.typ {
            // Anything that is in "result" will be ignored.
            return format!("ev#{}", ev.to_string());
        }

        match &self.result {
            Err(err) => format!("err#{}", err),
            Ok(opt) if opt.as_ref().is_some_and(|v| !v.is_empty()) => {
                format!("not#{}", opt.as_ref().clone().unwrap())
            }
            _ => "".to_owned(),
        }
    }
}

#[repr(u8)]
#[derive(Copy, Clone)]
pub enum Flags {
    NONE = 0,
    SILENT = 1,
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
        let (first, second) = data.split_once('#').ok_or(PayloadError::new(
            data.to_owned(),
            "'#' was not found in split.".to_owned(),
            String::new(),
        ))?;
        Ok(Desc::new(
            first.parse::<Pid>().map_err(|err| {
                PayloadError::new(
                    data.to_owned(),
                    "Could not parse pid.".to_owned(),
                    err.to_string(),
                )
            })?,
            second.parse::<FlagsRepr>().map_err(|err| {
                PayloadError::new(
                    data.to_owned(),
                    "Could not parse flags.".to_owned(),
                    err.to_string(),
                )
            })?,
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
        let (desc, job) = data.split_once("##").ok_or(PayloadError::new(
            data.to_owned(),
            "'##' was not found in split.".to_owned(),
            String::new(),
        ))?;

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
