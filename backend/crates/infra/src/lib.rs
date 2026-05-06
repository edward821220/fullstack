pub mod audit;
pub mod health_checker;
pub mod telemetry;

use config::AuditConfig;
use std::sync::Arc;

use audit::{NoopExporter, OtelLogsExporter, SyslogExporter};

/// Factory function for creating an audit exporter from configuration.
///
/// Shared between the REST and gRPC server binaries so the construction
/// logic is not duplicated.
pub fn create_audit_exporter(
    config: &AuditConfig,
    service_name: &str,
) -> Arc<dyn svc::audit::AuditExporter> {
    match config.exporter.as_str() {
        "syslog" => {
            let cfg = config.syslog.as_ref().expect("syslog config validated");
            Arc::new(SyslogExporter::new(
                cfg.host.clone(),
                cfg.port,
                cfg.protocol.clone(),
                &cfg.facility,
                service_name.to_owned(),
                std::env::var("HOSTNAME").unwrap_or_else(|_| "localhost".to_owned()),
                cfg.tls_enabled,
            ))
        }
        "otel-logs" => {
            let cfg = config
                .otel_logs
                .as_ref()
                .expect("otel_logs config validated");
            Arc::new(OtelLogsExporter::new(
                cfg.endpoint.clone(),
                cfg.timeout_seconds,
                service_name.to_owned(),
            ))
        }
        _ => Arc::new(NoopExporter),
    }
}
