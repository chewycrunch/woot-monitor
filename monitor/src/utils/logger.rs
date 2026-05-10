use chrono::Local;
use colored::Colorize;

pub enum LogLevel {
    Info,
    Warn,
    Error,
    New,
    Debug,
}

impl LogLevel {
    // fn as_str(&self) -> &'static str {
    //     match self {
    //         LogLevel::Info => "INFO",
    //         LogLevel::Warn => "WARN",
    //         LogLevel::Error => "ERROR",
    //         LogLevel::New => "NEW",
    //         LogLevel::Debug => "DEBUG",
    //     }
    // }
}

pub fn log(level: LogLevel, message: impl AsRef<str>) {
    let now = Local::now().format("%Y-%m-%d %H:%M:%S");
    let level_str = match level {
        LogLevel::Info => "INFO".blue(),
        LogLevel::Warn => "WARN".yellow(),
        LogLevel::Error => "ERROR".red(),
        LogLevel::New => "NEW".green(),
        LogLevel::Debug => "DEBUG".cyan(),
    };
    println!("[{}] [{}] {}", now, level_str, message.as_ref());
}
