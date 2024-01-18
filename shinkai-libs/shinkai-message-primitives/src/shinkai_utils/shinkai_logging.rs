use chrono::Local;
use colored::*;
use std::io::Write;
use syslog::{Logger, Facility};
use tracing::{span, Level, error, info, debug, instrument, Subscriber};
use tracing_subscriber::FmtSubscriber;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::EnvFilter;

// Note(Nico): Added this to avoid issues when running tests
use std::sync::Once;
static INIT: Once = Once::new();
static mut GUARD: Option<WorkerGuard> = None;

struct SyslogWriter {
    logger: syslog::Logger<syslog::LoggerBackend, syslog::Formatter3164>,
}

impl SyslogWriter {
    fn new(facility: Facility) -> Self {
        let formatter = syslog::Formatter3164 {
            facility: syslog::Facility::LOG_USER,
            hostname: Some(std::env::var("LOG_SYSLOG_SERVER").unwrap_or("localhost".to_string())),
            process: "shinkai_node".into(),
            pid: 0,
        };
        let logger = syslog::unix(formatter).expect("Could not connect to syslog");
        Self { logger }
    }
}

impl Write for SyslogWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        eprintln!("buf: {:?}", buf);
        let message_with_codes = String::from_utf8_lossy(buf);
        let message = strip_ansi_escapes::strip(message_with_codes.clone().into_owned().as_bytes())
            .unwrap_or_else(|_| message_with_codes.into_owned().into_bytes());
        let message = String::from_utf8_lossy(&message);
        eprintln!("message: {:?}", message);

        let mut parts = message.split(' ');
        parts.nth(3); // Skip the timestamp
        let log_level = parts.next().unwrap_or("");
        let log_message = parts.collect::<Vec<&str>>().join(" ");


        eprintln!("Parsed log level: {}", log_level);
        eprintln!("Parsed log message: {}", log_message);

        match log_level {
            "ERROR" => if let Err(e) = self.logger.err(log_message) {
                eprintln!("Failed to send ERROR log: {}", e);
            },
            "INFO" => if let Err(e) = self.logger.info(log_message) {
                eprintln!("Failed to send INFO log: {}", e);
            },
            "DEBUG" => if let Err(e) = self.logger.debug(log_message) {
                eprintln!("Failed to send DEBUG log: {}", e);
            },
            _ => if let Err(e) = self.logger.info(log_message) {
                eprintln!("Failed to send log: {}", e);
            }, // Default to INFO if no level is found
        }


        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

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
        let is_simple_log = std::env::var("LOG_SIMPLE").is_ok();
        let time = Local::now().format("%Y-%m-%d %H:%M:%S"); // Simplified timestamp

        let option_str = format!("{:?}", option);
        let level_str = match level {
            ShinkaiLogLevel::Error => "ERROR",
            ShinkaiLogLevel::Info => "INFO",
            ShinkaiLogLevel::Debug => "DEBUG",
        };

        let message_with_header = if is_simple_log {
            format!("{}", message)
        } else {
            let hostname = "localhost";
            let app_name = "shinkai";
            let proc_id = std::process::id().to_string();
            let msg_id = "-";
            let header = format!("{} {} {} {} {}", time, hostname, app_name, proc_id, msg_id);
            format!("{} - {} - {} - {}", header, level_str, option_str, message)
        };

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
        let syslog_server = std::env::var("LOG_SYSLOG_SERVER").unwrap_or("".to_string());
        let (non_blocking, guard) = if !syslog_server.is_empty() {
            let syslog_writer = SyslogWriter::new(syslog::Facility::LOG_USER);
            tracing_appender::non_blocking(syslog_writer)
        } else {
            tracing_appender::non_blocking(std::io::stdout())
        };

        let subscriber = FmtSubscriber::builder()
            .with_env_filter(EnvFilter::from_default_env())
            .with_writer(non_blocking)
            .finish();

        tracing::subscriber::set_global_default(subscriber)
            .expect("setting default subscriber failed");

        unsafe {
            GUARD = Some(guard);
        }
    });
}