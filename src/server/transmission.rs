use std::rc::Rc;
use std::string::ToString;

use super::jobs::{FulfilledJob, JobDesc, Pid};

trait PayloadIntern {
    fn from_str_box(self: Rc<Self>, data: &str)
    -> Result<Box<dyn Payload + 'static>, PayloadError>;
}

// Since we only implement Payload for concrete types and Box<dyn Paylaod>
// we justify this use of 'static. In addition, the method 'from_str_static'
// implies a concrete type. Anything else should fail.
impl<T> PayloadIntern for T
where
    T: Payload + 'static,
{
    fn from_str_box(
        self: Rc<Self>,
        data: &str,
    ) -> Result<Box<dyn Payload + 'static>, PayloadError> {
        <Self as Payload>::from_str_static(data)
            .map(|x| -> Box<dyn Payload + 'static> { Box::new(x) })
    }
}

pub trait Payload: PayloadIntern {
    fn from_str_static(data: &str) -> Result<Self, PayloadError>
    where
        Self: Sized;

    fn from_str(&self, data: &str) -> Result<Self, PayloadError>
    where
        Self: Sized,
    {
        <Self as Payload>::from_str_static(data)
    }

    fn to_string(&self) -> String;
}

impl Payload for Box<dyn Payload> {
    fn from_str_static(_: &str) -> Result<Self, PayloadError>
    where
        Self: Sized,
    {
        unreachable!("Someone thought it was a good idea to parse something concrete with dyn.");
    }

    fn from_str(&self, data: &str) -> Result<Self, PayloadError>
    where
        Self: Sized,
    {
        <dyn Payload as PayloadIntern>::from_str_box(
            unsafe { Rc::from_raw(&**self as *const dyn Payload) },
            data,
        )
    }

    fn to_string(&self) -> String {
        (**self).to_string()
    }
}

pub struct PayloadError {
    pub data: String,
    pub error: String,
    pub details: String,
}

impl PayloadError {
    pub const fn new(data: String, error: String, details: String) -> PayloadError {
        PayloadError {
            data,
            error,
            details,
        }
    }
}

pub struct Transmission {
    pub from: Pid,
    pub payload: Box<dyn Payload>,
}

impl Transmission {
    pub fn new(from: Pid, payload: Box<dyn Payload>) -> Transmission {
        Transmission { from, payload }
    }
}

impl Payload for Transmission {
    fn from_str_static(data: &str) -> Result<Transmission, PayloadError> {
        let (from, rest) = data.split_once("###").ok_or(PayloadError::new(
            data.to_owned(),
            "'###' was not found in split.".to_owned(),
            String::new(),
        ))?;

        let pid = from.parse::<Pid>().map_err(|err| {
            PayloadError::new(
                from.to_owned(),
                "Error while parsing 'from' in 'Transmission'".to_owned(),
                err.to_string(),
            )
        })?;

        Ok(Transmission::new(
            pid,
            match pid {
                0 => Box::new(FulfilledJob::from_str_static(rest)?),
                _ => Box::new(JobDesc::from_str_static(rest)?),
            },
        ))
    }

    fn to_string(&self) -> String {
        format!("{}###{}", self.from, self.payload.to_string())
    }
}

pub struct FulfilledTransmission {
    pub to: Pid,
    pub fulfilled: FulfilledJob,
}

impl FulfilledTransmission {
    pub fn new(to: Pid, fulfilled: FulfilledJob) -> FulfilledTransmission {
        FulfilledTransmission { to, fulfilled }
    }
}

impl Payload for FulfilledTransmission {
    fn from_str_static(data: &str) -> Result<Self, PayloadError> {
        let (to, ful) = data.split_once("#").ok_or(PayloadError::new(
            data.to_owned(),
            "'#' was not found in split.".to_owned(),
            String::new(),
        ))?;

        Ok(FulfilledTransmission::new(
            to.parse::<Pid>().map_err(|err| {
                PayloadError::new(
                    to.to_owned(),
                    "Error parsing 'to' for 'FulfilledTransmission'".to_owned(),
                    err.to_string(),
                )
            })?,
            FulfilledJob::from_str_static(ful)?,
        ))
    }

    fn to_string(&self) -> String {
        format!("{}#{}", self.to, self.fulfilled.to_string())
    }
}
