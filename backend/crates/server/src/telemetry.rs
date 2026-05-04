use std::time::Duration;

use config::{AppConfig, LogFormat, OtlpProtocol};
use opentelemetry::{KeyValue, global, trace::TracerProvider as _};
use opentelemetry_otlp::{Protocol, SpanExporter, WithExportConfig};
use opentelemetry_sdk::{Resource, propagation::TraceContextPropagator, trace::SdkTracerProvider};
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

pub struct TelemetryGuard {
    tracer_provider: Option<SdkTracerProvider>,
}

impl TelemetryGuard {
    pub fn shutdown(self) {
        if let Some(provider) = self.tracer_provider {
            let _ = provider.shutdown();
        }
    }
}

pub fn init_tracing(config: &AppConfig) -> Result<TelemetryGuard, String> {
    let tracer_provider = build_tracer_provider(config)?;

    match &tracer_provider {
        Some(provider) => init_subscriber_with_otel(config, provider)?,
        None => init_subscriber_without_otel(config)?,
    }

    Ok(TelemetryGuard { tracer_provider })
}

fn build_env_filter(config: &AppConfig) -> EnvFilter {
    EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(&config.observability.log_level))
}

fn build_resource(config: &AppConfig) -> Resource {
    Resource::builder_empty()
        .with_attributes([
            KeyValue::new("service.name", config.observability.service_name.clone()),
            KeyValue::new("service.version", env!("CARGO_PKG_VERSION")),
        ])
        .build()
}

fn build_tracer_provider(config: &AppConfig) -> Result<Option<SdkTracerProvider>, String> {
    if !config.observability.otlp.enabled {
        return Ok(None);
    }

    global::set_text_map_propagator(TraceContextPropagator::new());

    let exporter = match config.observability.otlp.protocol {
        OtlpProtocol::Http => SpanExporter::builder()
            .with_http()
            .with_protocol(Protocol::HttpBinary)
            .with_endpoint(config.observability.otlp.endpoint.clone())
            .with_timeout(Duration::from_secs(
                config.observability.otlp.timeout_seconds,
            ))
            .build()
            .map_err(|e| format!("Failed to build HTTP OTLP span exporter: {e}"))?,
        OtlpProtocol::Grpc => SpanExporter::builder()
            .with_tonic()
            .with_endpoint(config.observability.otlp.endpoint.clone())
            .with_timeout(Duration::from_secs(
                config.observability.otlp.timeout_seconds,
            ))
            .build()
            .map_err(|e| format!("Failed to build gRPC OTLP span exporter: {e}"))?,
    };

    let provider = SdkTracerProvider::builder()
        .with_resource(build_resource(config))
        .with_batch_exporter(exporter)
        .build();

    global::set_tracer_provider(provider.clone());

    Ok(Some(provider))
}

fn init_subscriber_with_otel(
    config: &AppConfig,
    provider: &SdkTracerProvider,
) -> Result<(), String> {
    let tracer = provider.tracer(config.observability.service_name.clone());

    match config.observability.log_format {
        LogFormat::Json => tracing_subscriber::registry()
            .with(build_env_filter(config))
            .with(fmt::layer().json())
            .with(tracing_opentelemetry::layer().with_tracer(tracer))
            .try_init()
            .map_err(|e| format!("Failed to initialize tracing subscriber: {e}")),
        LogFormat::Pretty => tracing_subscriber::registry()
            .with(build_env_filter(config))
            .with(fmt::layer().pretty())
            .with(tracing_opentelemetry::layer().with_tracer(tracer))
            .try_init()
            .map_err(|e| format!("Failed to initialize tracing subscriber: {e}")),
    }
}

fn init_subscriber_without_otel(config: &AppConfig) -> Result<(), String> {
    match config.observability.log_format {
        LogFormat::Json => tracing_subscriber::registry()
            .with(build_env_filter(config))
            .with(fmt::layer().json())
            .try_init()
            .map_err(|e| format!("Failed to initialize tracing subscriber: {e}")),
        LogFormat::Pretty => tracing_subscriber::registry()
            .with(build_env_filter(config))
            .with(fmt::layer().pretty())
            .try_init()
            .map_err(|e| format!("Failed to initialize tracing subscriber: {e}")),
    }
}
