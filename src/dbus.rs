use std::cell::OnceCell;
use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use zbus::{interface, object_server::SignalEmitter, zvariant::OwnedValue};

use crate::jobs::{Job, JobDescription};
use crate::logger::Logger;
use crate::manager::GLOBAL_MANAGER;

const CAPABILITIES: OnceCell<Vec<&'static str>> = OnceCell::new();

#[derive(Serialize, Deserialize)]
pub struct NotificationEvent {
    id: u32,
    app_name: String,
    replaces_id: u32,
    app_icon: String,
    summary: String,
    body: String,
    actions: Vec<String>,
    hints: Vec<HashMap<String, OwnedValue>>,
}

pub struct Notifications {
    counter: u32,
}

#[interface(name = "org.freedesktop.Notifications")]
impl Notifications {
    pub async fn get_capabilities(&self) -> Vec<&'static str> {
        CAPABILITIES.get().unwrap().clone()
    }

    pub async fn notify(
        &mut self,
        app_name: String,
        replaces_id: u32,
        app_icon: String,
        summary: String,
        body: String,
        actions: Vec<String>,
        hints: Vec<HashMap<String, OwnedValue>>,
    ) -> u32 {
        let id = if replaces_id == 0 {
            self.counter += 1;
            self.counter
        } else {
            replaces_id
        };

        push_manager(JobDescription::Broadcast {
            event: NotificationEvent {
                id,
                app_name,
                replaces_id,
                app_icon,
                summary,
                body,
                actions,
                hints,
            },
        });

        id
    }

    pub async fn close_notification(&self, id: u32) {
        push_manager(JobDescription::Close { pid: id });
    }

    #[zbus(out_args("name", "vendor", "version", "spec_version"))]
    pub async fn get_server_information(
        &self,
    ) -> zbus::fdo::Result<(&'static str, &'static str, &'static str, &'static str)> {
        Ok(("notifs-piper", "notifs-piper", "0.1.0", "1.3"))
    }

    #[zbus(signal)]
    pub async fn notification_closed(emitter: &SignalEmitter<'_>) -> zbus::Result<()>;

    #[zbus(signal)]
    pub async fn action_invoked(emitter: &SignalEmitter<'_>) -> zbus::Result<()>;

    #[zbus(signal)]
    pub async fn activation_token(emitter: &SignalEmitter<'_>) -> zbus::Result<()>;
}

fn push_manager(jobd: JobDescription) {
    match GLOBAL_MANAGER.write() {
        Err(poison) => {
            Logger::error(
                format!(
                    "{}\n{}\n{}",
                    "!!FATAL ERROR!! A thread panicked while holding the GLOBAL_MANAGER.",
                    format!("Source: {}", std::error::Error::source(&poison).unwrap()),
                    format!("Description: {}", poison),
                )
                .as_str(),
            );
            panic!();
        }
        Ok(mut manager) => {
            manager.job_list.push_back(Job::new(0, jobd));
        }
    }
}
