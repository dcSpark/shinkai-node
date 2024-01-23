use opentelemetry::{global, trace::Tracer as _, KeyValue};
use opentelemetry_otlp::new_pipeline;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::metrics::MeterProvider;
use opentelemetry_sdk::runtime;
use opentelemetry_sdk::trace as sdktrace;
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
use tonic::metadata::MetadataMap;
use tracing::Level;
use tracing_opentelemetry::OpenTelemetryLayer;
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

    println!("Telemetry data sent to the OpenTelemetry backend successfully.");
}

fn resource() -> Resource {
    Resource::new(vec![
        KeyValue::new(SERVICE_NAME, env!("CARGO_PKG_NAME")),
        KeyValue::new(SERVICE_VERSION, env!("CARGO_PKG_VERSION")),
        KeyValue::new(DEPLOYMENT_ENVIRONMENT, "develop"),
    ])
}

fn init_tracer(telemetry_endpoint: &str) -> Tracer {
    let auth_header = format!("Basic YXBtQHNoaW5rYWkuY29tOjBjSWpCWWVHdGFyTHdaRHE");

    let mut map = tonic::metadata::MetadataMap::with_capacity(3);
    map.insert("authorization", auth_header.parse().unwrap());
    map.insert("organization", "default".parse().unwrap());
    map.insert("stream-name", "default".parse().unwrap());

    opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_trace_config(
            Config::default()
                .with_sampler(Sampler::AlwaysOn)
                .with_resource(resource()),
        )
        .with_batch_config(BatchConfig::default())
        .with_exporter(
            opentelemetry_otlp::new_exporter()
                .tonic() // Use gRPC instead of HTTP
                .with_endpoint(telemetry_endpoint)
                .with_metadata(map)
                .with_timeout(Duration::from_secs(3)),
        )
        .install_batch(runtime::Tokio)
        .expect("Failed to install OpenTelemetry tracer.")
}
