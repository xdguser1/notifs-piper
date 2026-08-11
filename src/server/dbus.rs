use std::collections::HashMap;
use std::error::Error;
use std::sync::{Arc, mpsc::Sender};
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};
use zbus::{interface, object_server::SignalEmitter, zvariant::OwnedValue};

use crate::utils::logger::Logger;

use super::jobs::{Broadcast, Close, Desc, JobDesc, SyncList};

// TODO: Update capabilities
// TODO: Add signal management
const CAPABILITIES: [&'static str; 0] = [];

pub type Nid = u32;

#[derive(Serialize, Deserialize)]
pub struct NotificationEvent {
    id: Nid,
    time: u128,
    pub read: bool,
    app_name: String,
    replaces_id: Nid,
    app_icon: String,
    summary: String,
    body: String,
    actions: Vec<String>,
    hints: HashMap<String, OwnedValue>,
    timeout: i32,
}

impl NotificationEvent {
    pub const fn get_id(&self) -> Nid {
        self.id
    }

    pub const fn get_replacement(&self) -> Nid {
        self.replaces_id
    }
}

pub struct Notifications {
    counter: Nid,
    sync_list: SyncList,
    sender: Sender<()>,
}

unsafe impl Send for Notifications {}

unsafe impl Sync for Notifications {}

impl Notifications {
    fn push_job(&self, desc: JobDesc) {
        match self.sync_list.lock() {
            Err(poison) => {
                Logger::error(
                    format!(
                        "{}\n{}\n{}",
                        "!!FATAL ERROR!! A thread panicked while holding the LogManager.",
                        format!("Source: {}", Error::source(&poison).unwrap()),
                        format!("Description: {}", poison),
                    )
                    .as_str(),
                );
                panic!();
            }
            Ok(mut sync_list) => {
                Logger::cdebug("Job scheduled for execution.", None);
                sync_list.push_back(desc);
            }
        }
    }

    pub fn new(counter: Nid, sync_list: &SyncList, sender: Sender<()>) -> Notifications {
        Notifications {
            counter: counter,
            sync_list: Arc::clone(sync_list),
            sender,
        }
    }

    pub fn notify(
        &mut self,
        app_name: String,
        replaces_id: Nid,
        app_icon: String,
        summary: String,
        body: String,
        actions: Vec<String>,
        hints: HashMap<String, OwnedValue>,
        timeout: i32,
    ) -> Nid {
        Logger::cdebug("Received new notification. Creating job.", None);

        let id = if replaces_id == 0 {
            self.counter += 1;
            self.counter
        } else {
            replaces_id
        };

        self.push_job(JobDesc::new(
            Box::new(Broadcast::new(NotificationEvent {
                id,
                time: SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap_or(Duration::new(0, 0))
                    .as_millis(),
                read: false,
                app_name,
                replaces_id,
                app_icon,
                summary,
                body,
                actions,
                hints,
                timeout,
            })),
            Desc::new(0, 0),
        ));

        self.sender.send(()).unwrap();

        id
    }

    pub fn close_notification(&self, nid: Nid) {
        Logger::cdebug("Received closed command. Creating job.", None);
        self.push_job(JobDesc::new(Box::new(Close(nid)), Desc::new(0, 0)));
    }
}

pub struct NotificationsWrapper {
    pub inner: Notifications,
}

#[interface(name = "org.freedesktop.Notifications")]
impl NotificationsWrapper {
    pub async fn notify(
        &mut self,
        app_name: String,
        replaces_id: Nid,
        app_icon: String,
        summary: String,
        body: String,
        actions: Vec<String>,
        hints: HashMap<String, OwnedValue>,
        timeout: i32,
    ) -> u32 {
        self.inner.notify(
            app_name,
            replaces_id,
            app_icon,
            summary,
            body,
            actions,
            hints,
            timeout,
        )
    }

    pub async fn close_notification(&self, id: Nid) {
        self.inner.close_notification(id);
    }

    pub async fn get_capabilities(&self) -> Vec<&'static str> {
        CAPABILITIES.to_vec()
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
