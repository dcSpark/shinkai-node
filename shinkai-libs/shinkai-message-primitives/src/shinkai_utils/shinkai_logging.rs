use colored::*;
use chrono::Local;

#[derive(PartialEq)]
pub enum ShinkaiLogOption {
    Blockchain,
    Database,
    Identity,
    API,
    DetailedAPI,
    Node,
    InternalAPI,
    Tests,
}

#[derive(PartialEq)]
pub enum ShinkaiLogLevel {
    Error,
    Info,
    Debug,
}

impl ShinkaiLogLevel {
    fn to_log_level(&self) -> log::Level {
        match self {
            ShinkaiLogLevel::Error => log::Level::Error,
            ShinkaiLogLevel::Info => log::Level::Info,
            ShinkaiLogLevel::Debug => log::Level::Debug,
        }
    }
}

fn active_log_options() -> Vec<ShinkaiLogOption> {
    let mut active_options = Vec::new();
    if std::env::var("LOG_BLOCKCHAIN").is_ok() {
        active_options.push(ShinkaiLogOption::Blockchain);
    }
    if std::env::var("LOG_DATABASE").is_ok() {
        active_options.push(ShinkaiLogOption::Database);
    }
    if std::env::var("LOG_IDENTITY").is_ok() {
        active_options.push(ShinkaiLogOption::Identity);
    }
    if std::env::var("LOG_API").is_ok() {
        active_options.push(ShinkaiLogOption::API);
    }
    if std::env::var("LOG_DETAILED_API").is_ok() {
        active_options.push(ShinkaiLogOption::DetailedAPI);
    }
    if std::env::var("LOG_NODE").is_ok() {
        active_options.push(ShinkaiLogOption::Node);
    }
    if std::env::var("LOG_INTERNAL_API").is_ok() {
        active_options.push(ShinkaiLogOption::InternalAPI);
    }
    if std::env::var("LOG_TESTS").is_ok() {
        active_options.push(ShinkaiLogOption::Tests);
    }
    
    active_options
}

pub fn shinkai_log(option: ShinkaiLogOption, level: ShinkaiLogLevel, message: &str) {
    let active_options = active_log_options();
    if active_options.contains(&option) {
        let time = Local::now().format("%Y-%m-%d %H:%M:%S");
        let message_with_time = format!("{} | {}", time, message);
        match level.to_log_level() {
            log::Level::Error => eprintln!("{}", message_with_time.red()),
            log::Level::Info => println!("{}", message_with_time.yellow()),
            log::Level::Debug => println!("{}", message_with_time),
            _ => {},
        }
    }
}
