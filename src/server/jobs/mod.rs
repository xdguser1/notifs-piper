use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use super::manager::LogsManager;
use super::transmission::{Payload, PayloadError};

pub use self::broadcast::Broadcast;
pub use self::read::Read;
pub use self::watch::Watch;

mod broadcast;
mod read;
mod watch;

pub type FlagsRepr = u8;
pub type Pid = u32;
pub type JobList = VecDeque<JobDesc>;
pub type SyncList = Arc<Mutex<JobList>>;

pub trait Job: Payload {
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
        _ => unreachable!("Someone forgot to put their canonical name here."),
    })
}

pub struct FulfilledJob(pub Result<Option<String>, String>);

impl Payload for FulfilledJob {
    fn from_str_static(data: &str) -> Result<FulfilledJob, PayloadError> {
        if data.starts_with('!') {
            Ok(FulfilledJob(Err(data[1..].to_owned())))
        } else if data.is_empty() {
            Ok(FulfilledJob(Ok(None)))
        } else {
            Ok(FulfilledJob(Ok(Some(data.to_owned()))))
        }
    }

    fn to_string(&self) -> String {
        match &self.0 {
            Err(err) => format!("!{}", err),
            Ok(opt) => opt.clone().unwrap_or(String::new()),
        }
    }
}

#[repr(u8)]
#[derive(Copy, Clone)]
pub enum Flags {
    SILENT = 1,
}

impl Flags {
    pub const fn is(&self, int: FlagsRepr) -> bool {
        (*self as FlagsRepr) & int != 0
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
