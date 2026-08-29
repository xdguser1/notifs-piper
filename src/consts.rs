/***** DBUS CONSTANTS *****/
pub const VERSION: &str = "0.1.2";
pub const NAME: &str = "notifs-piper";
pub const NOTIFICATIONS_PROT_VER: &str = "1.3";

/***** SERVER CONSTANTS *****/
pub const CAPABILITIES_ENUMERATED: [&'static str; 10] = [
    "action-icons",
    "actions",
    "body",
    "body-hyperlinks",
    "body-images",
    "body-markup",
    "icon-multi",
    "icon-static",
    "persistence",
    "sound",
];
