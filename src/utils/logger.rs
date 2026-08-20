#![allow(dead_code)]

use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Copy, Clone)]
pub struct LogSettings {
    pub bold: bool,
    pub underline: bool,
    pub blinking: bool,
    pub reverse: bool,
    pub bg: Colors,
    pub fg: Colors,
}

impl LogSettings {
    #[inline(always)]
    pub const fn underline(&self) -> &'static str {
        if self.underline { "\x1b[4m" } else { "" }
    }

    #[inline(always)]
    pub const fn bold(&self) -> &'static str {
        if self.bold { "\x1b[1m" } else { "" }
    }

    #[inline(always)]
    pub const fn blinking(&self) -> &'static str {
        if self.blinking { "\x1b[5m" } else { "" }
    }

    #[inline(always)]
    pub const fn reverse(&self) -> &'static str {
        if self.reverse { "\x1b[7m" } else { "" }
    }

    #[inline]
    pub fn bash_settings(&self) -> String {
        self.bg.bg().to_owned()
            + self.fg.fg()
            + self.underline()
            + self.bold()
            + self.blinking()
            + self.reverse()
    }

    #[inline(always)]
    pub const fn reset() -> &'static str {
        Colors::reset()
    }
}

#[repr(usize)]
#[derive(Copy, Clone, Debug)]
pub enum Colors {
    BLACK = 0,
    RED = 1,
    GREEN = 2,
    YELLOW = 3,
    BLUE = 4,
    MAGENTA = 5,
    CYAN = 6,
    WHITE = 7,
    DEFAULT = 8,
}

impl Colors {
    const FG_STRINGS: [&str; 9] = [
        "\x1b[30m", "\x1b[31m", "\x1b[32m", "\x1b[33m", "\x1b[34m", "\x1b[35m", "\x1b[36m",
        "\x1b[37m", "\x1b[39m",
    ];

    const BG_STRINGS: [&str; 9] = [
        "\x1b[40m", "\x1b[41m", "\x1b[42m", "\x1b[43m", "\x1b[44m", "\x1b[45m", "\x1b[46m",
        "\x1b[47m", "\x1b[49m",
    ];

    pub const fn fg(&self) -> &str {
        Colors::FG_STRINGS[*self as usize]
    }

    pub const fn bg(&self) -> &str {
        Colors::BG_STRINGS[*self as usize]
    }

    pub const fn reset() -> &'static str {
        "\x1b[0m"
    }
}

pub struct Logger {
    pub settings: LogSettings,
}

impl Logger {
    const DEFAULT_SETTINGS: LogSettings = LogSettings {
        bold: false,
        underline: false,
        blinking: false,
        reverse: false,
        bg: Colors::DEFAULT,
        fg: Colors::WHITE,
    };

    const INFO_LOGGER: Logger = Logger::custom(&LogSettings {
        fg: Colors::CYAN,
        ..Logger::DEFAULT_SETTINGS
    });

    const WARN_LOGGER: Logger = Logger::custom(&LogSettings {
        fg: Colors::YELLOW,
        ..Logger::DEFAULT_SETTINGS
    });

    const ERROR_LOGGER: Logger = Logger::custom(&LogSettings {
        fg: Colors::RED,
        ..Logger::DEFAULT_SETTINGS
    });

    const DEBUG_LOGGER: Logger = Logger::custom(&LogSettings {
        fg: Colors::BLUE,
        ..Logger::DEFAULT_SETTINGS
    });

    #[inline(always)]
    pub const fn new() -> Logger {
        Logger::custom(&Logger::DEFAULT_SETTINGS)
    }

    #[inline(always)]
    pub const fn custom(settings: &LogSettings) -> Logger {
        Logger {
            settings: *settings,
        }
    }

    #[inline(always)]
    pub fn print(&self, text: &str) {
        print!(
            "{}{}{}",
            self.settings.bash_settings(),
            text,
            LogSettings::reset(),
        );
    }

    #[inline(always)]
    pub fn println(&self, text: &str) {
        self.print(text);
        println!();
    }

    #[inline(always)]
    pub fn err(&self, text: &str) {
        eprint!(
            "{}{}{}",
            self.settings.bash_settings(),
            text,
            LogSettings::reset(),
        );
    }

    #[inline(always)]
    pub fn errln(&self, text: &str) {
        self.err(text);
        eprintln!();
    }

    #[inline(always)]
    pub fn info(text: &str) {
        Logger::INFO_LOGGER.println(format!("[INFO]: {}", text).as_str());
    }

    #[inline(always)]
    pub fn warn(text: &str) {
        Logger::WARN_LOGGER.println(format!("[WARNING]: {}", text).as_str());
    }

    #[inline(always)]
    pub fn error(text: &str) {
        Logger::ERROR_LOGGER.errln(format!("[ERROR]: {}", text).as_str());
    }

    #[inline(always)]
    pub fn debug(text: &str) {
        Logger::DEBUG_LOGGER.println(format!("[DEBUG]: {}", text).as_str());
    }

    pub fn cdebug(text: &str, set: Option<bool>) {
        static SET: AtomicBool = AtomicBool::new(false);

        if set.is_some() {
            SET.store(set.unwrap(), Ordering::Release);
        }

        if SET.load(Ordering::Acquire) {
            Logger::debug(text);
        }
    }
}
