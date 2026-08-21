use super::jobs::{FulfilledJob, Pid};
use crate::utils::macros::parse::{parse, split_once};

pub trait Payload {
    fn from_str_static(data: &str) -> Result<Self, PayloadError>
    where
        Self: Sized;

    fn to_string(&self) -> String;
}

#[derive(Copy, Clone)]
pub enum TransmissionType {
    Incoming(Pid),
    Outgoing(Pid),
    Error,
}

impl Payload for TransmissionType {
    fn from_str_static(data: &str) -> Result<Self, PayloadError> {
        if data == "Err" {
            return Ok(TransmissionType::Error);
        }

        let (first, second) = split_once!(data, '#')?;
        let second = parse!(second, Pid, "TransmissionType")?;

        match first {
            "inc" => Ok(TransmissionType::Incoming(second)),
            "out" => Ok(TransmissionType::Outgoing(second)),
            _ => Err(PayloadError::new(
                first.to_owned(),
                "Invalid type for 'TransmissionType' enum.".to_owned(),
                String::new(),
            )),
        }
    }

    fn to_string(&self) -> String {
        match *self {
            TransmissionType::Incoming(pid) => format!("inc#{}", pid),
            TransmissionType::Outgoing(pid) => format!("out#{}", pid),
            TransmissionType::Error => "Err".to_owned(),
        }
    }
}

pub struct Transmission {
    pub typ: TransmissionType,
    pub data: String,
}

impl Transmission {
    pub const fn new(typ: TransmissionType, data: String) -> Transmission {
        Transmission { typ, data }
    }

    pub fn from_fulfilled(ft: &FulfilledJob, pid: Pid) -> Transmission {
        Transmission {
            typ: TransmissionType::Outgoing(pid),
            data: ft.to_string(),
        }
    }
}

impl Payload for Transmission {
    fn from_str_static(data: &str) -> Result<Self, PayloadError> {
        let (first, second) = split_once!(data, "##")?;

        Ok(Transmission {
            typ: TransmissionType::from_str_static(first)?,
            data: second.to_owned(),
        })
    }

    fn to_string(&self) -> String {
        format!("{}##{}", self.typ.to_string(), self.data)
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
