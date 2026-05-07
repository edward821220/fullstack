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

    pub fn level(&self) -> tracing::Level {
        match self {
            Self::AuthSuccess { .. }
            | Self::UserCreated { .. }
            | Self::UserUpdated { .. }
            | Self::UserDeleted { .. }
            | Self::UserProvisioned { .. } => tracing::Level::INFO,
            Self::AuthFailure { .. } | Self::RoleDenied { .. } => tracing::Level::WARN,
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

impl std::fmt::Display for AuditEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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
