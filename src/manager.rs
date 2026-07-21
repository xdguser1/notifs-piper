use std::collections::VecDeque;
use std::sync::RwLock;

use crate::jobs::Job;
use crate::listener::Listener;

pub static GLOBAL_MANAGER: RwLock<LogManager> = RwLock::new(LogManager::new());

pub struct LogManager {
    active_processes: Vec<u32>,
    pub job_list: VecDeque<Job>,
}

impl LogManager {
    pub const fn new() -> LogManager {
        LogManager {
            active_processes: Vec::new(),
            job_list: VecDeque::new(),
        }
    }
}
