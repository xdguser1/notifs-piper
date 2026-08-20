use std::collections::HashMap;
use std::error::Error;
use std::sync::{Arc, OnceLock, mpsc::Sender};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use zbus::{interface, object_server::SignalEmitter, zvariant::OwnedValue};

use crate::consts::{NAME, NOTIFICATIONS_PROT_VER, VERSION};
use crate::utils::{
    logger::Logger,
    macros::multithread::acquire_lock_panic,
};
use super::jobs::{
    Broadcast, Close, Desc, JobDesc, NotificationClosed, NotificationClosedRepr, SyncList,
};

pub static CAPABILITIES: OnceLock<Vec<&'static str>> = OnceLock::new();

pub type Nid = u32;

/// Represents a notification event.
/// The fields `time`, `read`, `closed` and `id` are managed internally.
/// The rest should not be written, since this is not this program's job.
#[derive(Serialize, Deserialize)]
pub struct NotificationEvent {
    id: Nid,
    time: u128,
    pub read: bool,
    closed: bool,
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
    #[inline(always)]
    pub const fn time(&self) -> u128 {
        self.time
    }

    #[inline(always)]
    pub const fn timeout(&self) -> i32 {
        self.timeout
    }

    #[inline(always)]
    pub const fn id(&self) -> Nid {
        self.id
    }

    #[inline(always)]
    pub const fn set_closed(&mut self) {
        self.closed = true;
    }

    #[inline(always)]
    pub const fn closed(&self) -> bool {
        self.closed
    }

    #[inline(always)]
    pub const fn replacement(&self) -> Nid {
        self.replaces_id
    }
}

pub struct Notifications {
    counter: Nid,
    list: SyncList,
    sender: Sender<()>,
}

// SAFETY: Notifications is safe to send because synchronization is done
// in the methods themselves.
unsafe impl Send for Notifications {}

// SAFETY: Notifications is safe to send because synchronization is done
// in the methods themselves.
unsafe impl Sync for Notifications {}

impl Notifications {
    fn push_job(&self, desc: JobDesc) {
        let mut lock = acquire_lock_panic!(self.list.lock(), "Notifications");
        lock.push_back(desc);
        self.sender.send(()).unwrap();
    }

    pub fn new(counter: Nid, list: SyncList, sender: Sender<()>) -> Notifications {
        Notifications { counter, list, sender, }
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
        Logger::cdebug("(DBUS THREAD): Received new notification.", None);

        let id = if replaces_id == 0 {
            // Do not change these lines, since they guarantee Nid is not 0
            self.counter += 1;
            self.counter
        } else {
            replaces_id
        };

        self.push_job(JobDesc::new(
            Box::new(Broadcast::new(NotificationEvent {
                id,
                time: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or(Duration::new(0, 0))
                    .as_millis(),
                read: false,
                closed: false,
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

        id
    }

    pub fn close_notification(&self, nid: Nid) {
        Logger::cdebug("(DBUS THREAD): Received closed command.", None);
        self.push_job(JobDesc::new(
            Box::new(Close::new(nid, NotificationClosed::CallCloseNotification)),
            Desc::new(0, 0),
        ));
    }
}

pub struct NotificationsWrapper {
    pub inner: Notifications,
}

#[interface(name = "org.freedesktop.Notifications", introspection_docs = false)]
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
        CAPABILITIES.get().unwrap().clone()
    }

    #[zbus(out_args("name", "vendor", "version", "spec_version"))]
    pub async fn get_server_information(
        &self,
    ) -> zbus::fdo::Result<(&'static str, &'static str, &'static str, &'static str)> {
        Ok((NAME, NAME, VERSION, NOTIFICATIONS_PROT_VER))
    }

    #[zbus(signal)]
    pub async fn notification_closed(
        emitter: &SignalEmitter<'_>,
        id: Nid,
        reason: NotificationClosedRepr,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    pub async fn action_invoked(
        emitter: &SignalEmitter<'_>,
        id: Nid,
        action_key: &str,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    pub async fn activation_token(
        emitter: &SignalEmitter<'_>,
        id: Nid,
        activation_token: &str,
    ) -> zbus::Result<()>;
}
