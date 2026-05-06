pub mod channel;
pub mod event;

pub use channel::{AuditDispatcher, AuditExporter, AuditMetrics};
pub use event::{AuditError, AuditEvent, PiiMode};

/// Facade that combines PII redaction with async dispatch.
///
/// This is the public interface consumed by handlers and middleware.
/// Construction still takes an exporter and PII mode so callers do not need
/// to know about the internal `AuditDispatcher`.
pub struct AuditService {
    dispatcher: AuditDispatcher,
    pii_mode: PiiMode,
}

impl AuditService {
    pub fn new(exporter: std::sync::Arc<dyn AuditExporter>, pii_mode: PiiMode) -> Self {
        Self {
            dispatcher: AuditDispatcher::new(exporter),
            pii_mode,
        }
    }

    pub fn record(&self, event: AuditEvent) {
        let event = if self.pii_mode == PiiMode::Redact {
            event.redacted()
        } else {
            event
        };
        self.dispatcher.send(event);
    }

    pub fn metrics(&self) -> &AuditMetrics {
        self.dispatcher.metrics()
    }
}

/// Writes directly to tracing.
pub fn log_audit_event(event: &AuditEvent) {
    match event.level() {
        tracing::Level::INFO => tracing::info!(target: "audit", "{event}"),
        tracing::Level::WARN => tracing::warn!(target: "audit", "{event}"),
        tracing::Level::ERROR => tracing::error!(target: "audit", "{event}"),
        _ => tracing::debug!(target: "audit", "{event}"),
    }
}
