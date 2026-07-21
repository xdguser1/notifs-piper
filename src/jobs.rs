use crate::dbus::NotificationEvent;

pub enum JobDescription {
    Read { start: u32, end: u32 },
    Watch,
    Broadcast { event: NotificationEvent },
    Close { pid: u32 },
    EmitSignal { event: String, params: Vec<String> },
}

pub struct Job {
    pid: u32,
    description: JobDescription,
}

impl Job {
    pub fn new(pid: u32, description: JobDescription) -> Job {
        Job { pid, description }
    }
}
