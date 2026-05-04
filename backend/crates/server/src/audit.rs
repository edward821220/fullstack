use std::fmt;
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
        }
    }
}

impl AuditEvent {
    pub fn level(&self) -> Level {
        match self {
            Self::AuthSuccess { .. }
            | Self::UserCreated { .. }
            | Self::UserUpdated { .. }
            | Self::UserDeleted { .. } => Level::INFO,
            Self::AuthFailure { .. } | Self::RoleDenied { .. } => Level::WARN,
        }
    }
}

pub fn log_audit_event(event: &AuditEvent) {
    match event.level() {
        Level::INFO => tracing::info!("{event}"),
        Level::WARN => tracing::warn!("{event}"),
        Level::ERROR => tracing::error!("{event}"),
        _ => tracing::debug!("{event}"),
    }
}
