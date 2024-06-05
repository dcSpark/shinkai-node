use opentelemetry::{trace::Tracer as _, KeyValue};

use opentelemetry_otlp::WithExportConfig;

use opentelemetry_sdk::runtime;

use opentelemetry_sdk::trace::BatchConfig;
use opentelemetry_sdk::trace::Config;
use opentelemetry_sdk::trace::Sampler;
use opentelemetry_sdk::trace::Tracer;
use opentelemetry_sdk::Resource;
use opentelemetry_semantic_conventions::resource::{DEPLOYMENT_ENVIRONMENT, SERVICE_NAME, SERVICE_VERSION};
use shinkai_message_primitives::shinkai_utils::shinkai_logging::ShinkaiLogLevel;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::ShinkaiLogOption;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::ShinkaiTelemetry;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::Registry;

struct OpenTelemetryLogger {
    tracer: Tracer,
}

impl ShinkaiTelemetry for OpenTelemetryLogger {
    fn log(&self, option: ShinkaiLogOption, level: ShinkaiLogLevel, message: &str) {
        let span = tracing::span!(tracing::Level::INFO, "span", option = tracing::field::debug(option));
        let _enter = span.enter();
        match level {
            ShinkaiLogLevel::Error => tracing::error!("{}", message),
            ShinkaiLogLevel::Info => tracing::info!("{}", message),
            ShinkaiLogLevel::Debug => tracing::debug!("{}", message),
        };
    }
}

pub fn init_telemetry_tracing(telemetry_endpoint: &str) {
    let tracer = init_tracer(telemetry_endpoint);
    let telemetry = tracing_opentelemetry::layer().with_tracer(tracer.clone());
    let subscriber = Registry::default().with(telemetry);
    
    tracing::subscriber::set_global_default(subscriber).expect("Failed to set global default subscriber");

    // Set the OpenTelemetryLogger as the ShinkaiTelemetry implementation
    let logger = Arc::new(OpenTelemetryLogger { tracer });
    shinkai_message_primitives::shinkai_utils::shinkai_logging::set_telemetry(logger);
}

fn resource() -> Resource {
    Resource::new(vec![
        KeyValue::new(SERVICE_NAME, env!("CARGO_PKG_NAME")),
        KeyValue::new(SERVICE_VERSION, env!("CARGO_PKG_VERSION")),
        KeyValue::new(DEPLOYMENT_ENVIRONMENT, "develop"),
    ])
}

fn init_tracer(telemetry_endpoint: &str) -> Tracer {
    let mut headers = HashMap::new();
    let auth_header = std::env::var("TELEMETRY_AUTH_HEADER").unwrap_or_else(|_| panic!("TELEMETRY_AUTH_HEADER not set"));
    headers.insert("Authorization".to_string(), auth_header);
    headers.insert("stream-name".to_string(), "default".to_string());

    let tracer = opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_trace_config(
            Config::default()
                .with_sampler(Sampler::AlwaysOn)
                .with_resource(resource()),
        )
        .with_batch_config(BatchConfig::default())
        .with_exporter(
            opentelemetry_otlp::new_exporter()
                .http()
                .with_endpoint(telemetry_endpoint)
                .with_headers(headers)
                .with_timeout(Duration::from_secs(3))
                // .with_tls_config(None), // Disable TLS
        )
        .install_batch(runtime::Tokio);

        match tracer {
            Ok(t) => {
                t
            }
            Err(e) => {
                eprintln!("Failed to install OpenTelemetry tracer: {}", e);
                panic!("Failed to install OpenTelemetry tracer");
            }
        }
}
