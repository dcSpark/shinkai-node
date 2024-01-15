use chrono::Local;
use colored::*;
use tracing::{span, Level, error, info, debug, instrument};
use tracing_subscriber::FmtSubscriber;

// Note(Nico): Added this to avoid issues when running tests
use std::sync::Once;
static INIT: Once = Once::new();

#[derive(PartialEq, Debug)]
pub enum ShinkaiLogOption {
    Blockchain,
    Database,
    Identity,
    CryptoIdentity,
    JobExecution,
    CronExecution,
    API,
    WsAPI,
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
    fn to_log_level(&self) -> Level {
        match self {
            ShinkaiLogLevel::Error => Level::ERROR,
            ShinkaiLogLevel::Info => Level::INFO,
            ShinkaiLogLevel::Debug => Level::DEBUG,
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
            ShinkaiLogOption::WsAPI,
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
    if std::env::var("LOG_WS_API").is_ok() {
        active_options.push(ShinkaiLogOption::WsAPI);
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

        // Create a span with a specified level.
        let span = match level {
            ShinkaiLogLevel::Error => span!(Level::ERROR, "{}", option_str),
            ShinkaiLogLevel::Info => span!(Level::INFO, "{}", option_str),
            ShinkaiLogLevel::Debug => span!(Level::DEBUG, "{}", option_str),
        };
        let _enter = span.enter();

        match level {
            ShinkaiLogLevel::Error => error!("{}", message_with_header),
            ShinkaiLogLevel::Info => info!("{}", message_with_header),
            ShinkaiLogLevel::Debug => debug!("{}", message_with_header),
        }
    }
}

pub fn init_tracing() {
    INIT.call_once(|| {
        let subscriber = FmtSubscriber::builder()
            .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .finish();

        tracing::subscriber::set_global_default(subscriber)
            .expect("setting default subscriber failed");
    });
}