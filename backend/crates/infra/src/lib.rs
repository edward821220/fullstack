pub mod audit;
pub mod health_checker;
pub mod startup;
pub mod telemetry;

use config::AuditConfig;
use std::sync::{Arc, Once};

use audit::{NoopExporter, OtelLogsExporter, SyslogExporter};

static CRYPTO_PROVIDER_INIT: Once = Once::new();

/// Install crypto providers once per process:
/// 1. jsonwebtoken crypto provider for JWT verification.
/// 2. rustls ring crypto provider for tonic (gRPC) TLS and other rustls usage.
pub fn ensure_jwt_crypto_provider() {
    CRYPTO_PROVIDER_INIT.call_once(|| {
        let result = jsonwebtoken::crypto::rust_crypto::DEFAULT_PROVIDER.install_default();
        debug_assert!(
            result.is_ok(),
            "jsonwebtoken crypto provider install should happen once"
        );
        let _ = tokio_rustls::rustls::crypto::ring::default_provider().install_default();
    });
}

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
