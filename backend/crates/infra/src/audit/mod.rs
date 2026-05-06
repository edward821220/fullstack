use svc::audit::{AuditError, AuditEvent, AuditExporter};

pub mod otel_logs;
pub mod proxy;
pub mod syslog;

pub use otel_logs::OtelLogsExporter;
pub use proxy::{AuditEventCtx, AuditEventProxy};
pub use syslog::SyslogExporter;

/// No-op exporter for when audit is disabled.
pub struct NoopExporter;

#[async_trait::async_trait]
impl AuditExporter for NoopExporter {
    async fn export(&self, _event: AuditEvent) -> Result<(), AuditError> {
        Ok(())
    }
}
