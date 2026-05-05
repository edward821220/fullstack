use std::fmt;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::Level;

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

pub struct AuditService {
    sender: mpsc::UnboundedSender<AuditEvent>,
}

impl AuditService {
    pub fn new(exporter: Arc<dyn AuditExporter>) -> Self {
        let (sender, mut receiver) = mpsc::unbounded_channel();

        tokio::spawn(async move {
            while let Some(event) = receiver.recv().await {
                if let Err(e) = exporter.export(event).await {
                    tracing::error!("Audit export failed: {e}");
                }
            }
        });

        Self { sender }
    }

    pub fn record(&self, event: AuditEvent) {
        if let Err(e) = self.sender.send(event) {
            tracing::error!("Audit channel closed, event dropped: {e}");
        }
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
