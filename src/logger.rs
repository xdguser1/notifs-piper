#[derive(Copy, Clone, Debug)]
pub struct LogSettings {
    bold: bool,
    underline: bool,
    blinking: bool,
    reverse: bool,
    bg: Colors,
    fg: Colors,
}

impl LogSettings {
    fn underline(&self) -> &'static str {
        if self.underline {
            "\\e[4m"
        } else {
            ""
        }
    }

    fn bold(&self) -> &'static str {
        if self.bold {
            "\\e[1m"
        } else {
            ""
        }
    }

    fn blinking(&self) -> &'static str {
        if self.blinking {
            "\\e[5m"
        } else {
            ""
        }
    }

    fn reverse(&self) -> &'static str {
        if self.reverse {
            "\\e[7m"
        } else {
            ""
        }
    }

    pub fn bash_settings(&self) -> String {
        self.bg.bg().to_owned()
            + self.fg.fg()
            + self.underline()
            + self.bold()
            + self.blinking()
            + self.reverse()
    }

    pub fn reset() -> &'static str {
        Colors::reset()
    }
}

#[derive(Copy, Clone, Debug)]
pub enum Colors {
    BLACK = 0,
    RED = 1,
    GREEN = 2,
    BROWN = 3,
    BLUE = 4,
    PURPLE = 5,
    CYAN = 6,
    LIGHT_GRAY = 7,
    DARK_GRAY = 8,
    LIGHT_RED = 9,
    LIGHT_GREEN = 10,
    YELLOW = 11,
    LIGHT_BLUE = 12,
    LIGHT_PURPLE = 13,
    LIGHT_CYAN = 14,
    WHITE = 15,
}

impl Colors {
    const FG_STRINGS: [&str; 16] = [
        "\\e[30m",
        "\\e[31m",
        "\\e[32m",
        "\\e[33m",
        "\\e[34m",
        "\\e[35m",
        "\\e[36m",
        "\\e[37m",
        "\\e[1;30m",
        "\\e[1;31m",
        "\\e[1;32m",
        "\\e[1;33m",
        "\\e[1;34m",
        "\\e[1;35m",
        "\\e[1;36m",
        "\\e[1;37m",
    ];

    const BG_STRINGS: [&str; 16] = [
        "\\e[40m",
        "\\e[41m",
        "\\e[42m",
        "\\e[43m",
        "\\e[44m",
        "\\e[45m",
        "\\e[46m",
        "\\e[47m",
        "\\e[1;40m",
        "\\e[1;41m",
        "\\e[1;42m",
        "\\e[1;43m",
        "\\e[1;44m",
        "\\e[1;45m",
        "\\e[1;46m",
        "\\e[1;47m",
    ];

    pub fn fg(&self) -> &str {
        Colors::FG_STRINGS[*self as usize]
    }

    pub fn bg(&self) -> &str {
        Colors::BG_STRINGS[*self as usize]
    }

    pub fn reset() -> &'static str {
        "\\e[0m"
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Logger {
    settings: LogSettings,
}

impl Logger {
    const DEFAULT_SETTINGS: LogSettings = LogSettings {
        bold: false,
        underline: false,
        blinking: false,
        reverse: false,
        bg: Colors::BLACK,
        fg: Colors::WHITE,
    };

    const INFO_LOGGER: Logger = Logger::custom(&LogSettings {
        fg: Colors::LIGHT_BLUE,
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

    pub fn new() -> Logger {
        Logger {
            settings: Logger::DEFAULT_SETTINGS.clone(),
        }
    }

    pub const fn custom(settings: &LogSettings) -> Logger {
        Logger {
            settings: *settings,
        }
    }

    pub fn print(&self, text: &str) {
        print!(
            "{}{}{}",
            self.settings.bash_settings(),
            text,
            LogSettings::reset()
        );
    }

    pub fn println(&self, text: &str) {
        println!(
            "{}{}{}",
            self.settings.bash_settings(),
            text,
            LogSettings::reset()
        );
    }

    pub fn info(text: &str) {
        Logger::INFO_LOGGER.println(format!("[INFO]: {}", text).as_str());
    }

    pub fn warn(text: &str) {
        Logger::WARN_LOGGER.println(format!("[WARNING]: {}", text).as_str());
    }

    pub fn error(text: &str) {
        Logger::ERROR_LOGGER.println(format!("[ERROR]: {}", text).as_str());
    }

    pub fn debug(text: &str) {
        Logger::DEBUG_LOGGER.println(format!("[DEBUG]: {}", text).as_str());
    }
}
