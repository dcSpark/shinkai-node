use chrono::Local;
use colored::*;

#[derive(PartialEq, Debug)]
pub enum ShinkaiLogOption {
    Blockchain,
    Database,
    Identity,
    CryptoIdentity,
    JobExecution,
    CronExecution,
    API,
    DetailedAPI,
    Node,
    InternalAPI,
    Network,
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
    if std::env::var("LOG_ALL").is_ok() {
        return vec![
            ShinkaiLogOption::Blockchain,
            ShinkaiLogOption::Database,
            ShinkaiLogOption::Identity,
            ShinkaiLogOption::CryptoIdentity,
            ShinkaiLogOption::JobExecution,
            ShinkaiLogOption::CronExecution,
            ShinkaiLogOption::API,
            ShinkaiLogOption::DetailedAPI,
            ShinkaiLogOption::Node,
            ShinkaiLogOption::InternalAPI,
            ShinkaiLogOption::Network,
            ShinkaiLogOption::Tests,
        ];
    }

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
    if std::env::var("LOG_CRYPTO_IDENTITY").is_ok() {
        active_options.push(ShinkaiLogOption::CryptoIdentity);
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
    if std::env::var("LOG_INTERNAL_NETWORK").is_ok() {
        active_options.push(ShinkaiLogOption::Network);
    }
    if std::env::var("LOG_TESTS").is_ok() {
        active_options.push(ShinkaiLogOption::Tests);
    }
    if std::env::var("LOG_JOB_EXECUTION").is_ok() {
        active_options.push(ShinkaiLogOption::JobExecution);
    }
    if std::env::var("LOG_CRON_EXECUTION").is_ok() {
        active_options.push(ShinkaiLogOption::CronExecution);
    }
    active_options
}

pub fn shinkai_log(option: ShinkaiLogOption, level: ShinkaiLogLevel, message: &str) {
    let active_options = active_log_options();
    if active_options.contains(&option) {
        let time = Local::now().format("%Y-%m-%dT%H:%M:%S%.fZ"); // RFC 3339 timestamp
        let option_str = format!("{:?}", option);

        let (level_str, color_fn): (&str, Box<dyn Fn(&str) -> ColoredString>) = match level {
            ShinkaiLogLevel::Error => ("(ERROR)", Box::new(|s: &str| s.red())),
            ShinkaiLogLevel::Info => ("(INFO)", Box::new(|s: &str| s.yellow())),
            ShinkaiLogLevel::Debug => ("(DEBUG)", Box::new(|s: &str| s.normal())),
        };

        let message_with_header = if std::env::var("LOG_SIMPLE").is_ok() {
            format!("{} {} - {} - {}", time, level_str, option_str, message)
        } else {
            let hostname = "localhost";
            let app_name = "shinkai";
            let proc_id = std::process::id().to_string();
            let msg_id = "-"; // No specific message ID
            let header = format!("{} {} {} {} {}", time, hostname, app_name, proc_id, msg_id);
            format!("{} - {} - {} - {}", header, level_str, option_str, message)
        };

        match level.to_log_level() {
            log::Level::Error => eprintln!("{}", color_fn(&message_with_header)),
            log::Level::Info => println!("{}", color_fn(&message_with_header)),
            log::Level::Debug => println!("{}", color_fn(&message_with_header)),
            _ => {}
        }
    }
}
