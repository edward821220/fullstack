use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::mpsc;
use tracing::Level;

/// Controls how PII is handled in audit events.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub enum PiiMode {
    /// Emit PII in clear text.
    #[default]
    Full,
    /// Mask email and sub fields.
    Redact,
}

#[derive(Debug, Clone)]
pub enum AuditEvent {
    AuthSuccess {
        user_id: uuid::Uuid,
        email: String,
        role: String,
        sub: String,
    },
    AuthFailure {
        reason: String,
    },
    RoleDenied {
        user_id: uuid::Uuid,
        required_role: String,
        actual_role: String,
    },
    UserCreated {
        actor_id: uuid::Uuid,
        created_id: uuid::Uuid,
        email: String,
    },
    UserUpdated {
        actor_id: uuid::Uuid,
        target_id: uuid::Uuid,
    },
    UserDeleted {
        actor_id: uuid::Uuid,
        target_id: uuid::Uuid,
    },
    UserProvisioned {
        user_id: uuid::Uuid,
        email: String,
        role: String,
    },
}

/// Redact an email address: `alice@example.com` -> `a***@example.com`.
fn redact_email(email: &str) -> String {
    if let Some(at) = email.find('@') {
        let local = &email[..at];
        let domain = &email[at..];
        let prefix = local.chars().next().unwrap_or('*');
        format!("{prefix}***{domain}")
    } else {
        "***".to_owned()
    }
}

/// Redact a subject claim: `auth0|123456` -> `auth0|***`.
fn redact_sub(sub: &str) -> String {
    if let Some(sep) = sub.rfind('|').or_else(|| sub.rfind(':')) {
        format!("{}***", &sub[..=sep])
    } else {
        "***".to_owned()
    }
}

impl AuditEvent {
    pub fn event_type(&self) -> &'static str {
        match self {
            Self::AuthSuccess { .. } => "auth_success",
            Self::AuthFailure { .. } => "auth_failure",
            Self::RoleDenied { .. } => "role_denied",
            Self::UserCreated { .. } => "user_created",
            Self::UserUpdated { .. } => "user_updated",
            Self::UserDeleted { .. } => "user_deleted",
            Self::UserProvisioned { .. } => "user_provisioned",
        }
    }

    pub fn level(&self) -> Level {
        match self {
            Self::AuthSuccess { .. }
            | Self::UserCreated { .. }
            | Self::UserUpdated { .. }
            | Self::UserDeleted { .. }
            | Self::UserProvisioned { .. } => Level::INFO,
            Self::AuthFailure { .. } | Self::RoleDenied { .. } => Level::WARN,
        }
    }

    /// Return a copy of this event with PII fields redacted.
    pub fn redacted(&self) -> Self {
        match self.clone() {
            Self::AuthSuccess {
                user_id,
                email,
                role,
                sub,
            } => Self::AuthSuccess {
                user_id,
                email: redact_email(&email),
                role,
                sub: redact_sub(&sub),
            },
            Self::UserCreated {
                actor_id,
                created_id,
                email,
            } => Self::UserCreated {
                actor_id,
                created_id,
                email: redact_email(&email),
            },
            Self::UserProvisioned {
                user_id,
                email,
                role,
            } => Self::UserProvisioned {
                user_id,
                email: redact_email(&email),
                role,
            },
            other => other,
        }
    }
}

impl fmt::Display for AuditEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AuthSuccess {
                user_id,
                email,
                role,
                ..
            } => write!(
                f,
                "auth_success user_id={user_id} email={email} role={role}"
            ),
            Self::AuthFailure { reason } => write!(f, "auth_failure reason={reason}"),
            Self::RoleDenied {
                user_id,
                required_role,
                actual_role,
            } => write!(
                f,
                "role_denied user_id={user_id} required={required_role} actual={actual_role}"
            ),
            Self::UserCreated {
                actor_id,
                created_id,
                email,
            } => write!(
                f,
                "user_created actor_id={actor_id} created_id={created_id} email={email}"
            ),
            Self::UserUpdated {
                actor_id,
                target_id,
            } => write!(f, "user_updated actor_id={actor_id} target_id={target_id}"),
            Self::UserDeleted {
                actor_id,
                target_id,
            } => write!(f, "user_deleted actor_id={actor_id} target_id={target_id}"),
            Self::UserProvisioned {
                user_id,
                email,
                role,
            } => write!(
                f,
                "user_provisioned user_id={user_id} email={email} role={role}"
            ),
        }
    }
}

#[derive(Debug, snafu::Snafu)]
pub enum AuditError {
    #[snafu(display("Export failed: {message}"))]
    Export { message: String },
}

#[async_trait::async_trait]
pub trait AuditExporter: Send + Sync {
    async fn export(&self, event: AuditEvent) -> Result<(), AuditError>;
}

/// Metrics for audit delivery health.
#[derive(Debug, Default)]
pub struct AuditMetrics {
    pub events_dropped: AtomicU64,
    pub exports_failed: AtomicU64,
}

pub struct AuditService {
    sender: mpsc::Sender<AuditEvent>,
    metrics: Arc<AuditMetrics>,
    pii_mode: PiiMode,
}

impl AuditService {
    pub fn new(exporter: Arc<dyn AuditExporter>, pii_mode: PiiMode) -> Self {
        // Bounded channel provides backpressure; excess events are dropped.
        const CHANNEL_CAPACITY: usize = 10_000;
        let (sender, mut receiver) = mpsc::channel::<AuditEvent>(CHANNEL_CAPACITY);
        let metrics = Arc::new(AuditMetrics::default());
        let metrics_clone = Arc::clone(&metrics);

        tokio::spawn(async move {
            while let Some(event) = receiver.recv().await {
                let mut retries = 0;
                let max_retries = 3;
                let mut delay_ms = 100;

                loop {
                    match exporter.export(event.clone()).await {
                        Ok(()) => break,
                        Err(e) => {
                            retries += 1;
                            if retries > max_retries {
                                tracing::error!(
                                    "Audit export failed after {max_retries} retries: {e}"
                                );
                                metrics_clone.exports_failed.fetch_add(1, Ordering::Relaxed);
                                break;
                            }
                            tracing::warn!(
                                "Audit export failed (retry {retries}/{max_retries}): {e}"
                            );
                            tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                            delay_ms *= 2;
                        }
                    }
                }
            }
        });

        Self {
            sender,
            metrics,
            pii_mode,
        }
    }

    pub fn record(&self, event: AuditEvent) {
        let event = if self.pii_mode == PiiMode::Redact {
            event.redacted()
        } else {
            event
        };

        match self.sender.try_send(event) {
            Ok(()) => {}
            Err(mpsc::error::TrySendError::Full(_)) => {
                let dropped = self.metrics.events_dropped.fetch_add(1, Ordering::Relaxed) + 1;
                tracing::warn!("Audit channel full, event dropped (total_dropped={dropped})");
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                tracing::error!("Audit channel closed, event dropped");
            }
        }
    }

    pub fn metrics(&self) -> &AuditMetrics {
        &self.metrics
    }
}

/// Writes directly to tracing.
pub fn log_audit_event(event: &AuditEvent) {
    match event.level() {
        Level::INFO => tracing::info!(target: "audit", "{event}"),
        Level::WARN => tracing::warn!(target: "audit", "{event}"),
        Level::ERROR => tracing::error!(target: "audit", "{event}"),
        _ => tracing::debug!(target: "audit", "{event}"),
    }
}
