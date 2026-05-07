use serde::Serialize;
use svc::audit::AuditEvent;

/// A serializable proxy for AuditEvent.
#[derive(Debug, Clone, Serialize)]
pub struct AuditEventProxy {
    pub event_type: String,
    pub user_id: Option<String>,
    pub email: Option<String>,
    pub role: Option<String>,
    pub sub: Option<String>,
    pub reason: Option<String>,
    pub required_role: Option<String>,
    pub actual_role: Option<String>,
    pub actor_id: Option<String>,
    pub target_id: Option<String>,
    pub created_id: Option<String>,
}

impl From<&AuditEvent> for AuditEventProxy {
    fn from(event: &AuditEvent) -> Self {
        let mut proxy = AuditEventProxy {
            event_type: event.event_type().to_owned(),
            user_id: None,
            email: None,
            role: None,
            sub: None,
            reason: None,
            required_role: None,
            actual_role: None,
            actor_id: None,
            target_id: None,
            created_id: None,
        };
        match event {
            AuditEvent::AuthSuccess {
                user_id,
                email,
                role,
                sub,
            } => {
                proxy.user_id = Some(user_id.to_string());
                proxy.email = Some(email.clone());
                proxy.role = Some(role.clone());
                proxy.sub = Some(sub.clone());
            }
            AuditEvent::AuthFailure { reason } => {
                proxy.reason = Some(reason.clone());
            }
            AuditEvent::RoleDenied {
                user_id,
                required_role,
                actual_role,
            } => {
                proxy.user_id = Some(user_id.to_string());
                proxy.required_role = Some(required_role.clone());
                proxy.actual_role = Some(actual_role.clone());
            }
            AuditEvent::UserCreated {
                actor_id,
                created_id,
                email,
            } => {
                proxy.actor_id = Some(actor_id.to_string());
                proxy.created_id = Some(created_id.to_string());
                proxy.email = Some(email.clone());
            }
            AuditEvent::UserUpdated {
                actor_id,
                target_id,
            } => {
                proxy.actor_id = Some(actor_id.to_string());
                proxy.target_id = Some(target_id.to_string());
            }
            AuditEvent::UserDeleted {
                actor_id,
                target_id,
            } => {
                proxy.actor_id = Some(actor_id.to_string());
                proxy.target_id = Some(target_id.to_string());
            }
            AuditEvent::UserProvisioned {
                user_id,
                email,
                role,
            } => {
                proxy.user_id = Some(user_id.to_string());
                proxy.email = Some(email.clone());
                proxy.role = Some(role.clone());
            }
        }
        proxy
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct AuditEventCtx {
    #[serde(flatten)]
    pub event: AuditEventProxy,
    pub timestamp: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_ip: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_agent: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_uuid() -> uuid::Uuid {
        uuid::Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap()
    }

    #[test]
    fn auth_success_maps_fields() {
        let event = AuditEvent::AuthSuccess {
            user_id: sample_uuid(),
            email: "alice@example.com".to_owned(),
            role: "admin".to_owned(),
            sub: "auth0|123".to_owned(),
        };
        let proxy = AuditEventProxy::from(&event);
        assert_eq!(proxy.event_type, "auth_success");
        assert_eq!(proxy.user_id, Some(sample_uuid().to_string()));
        assert_eq!(proxy.email, Some("alice@example.com".to_owned()));
        assert_eq!(proxy.role, Some("admin".to_owned()));
        assert_eq!(proxy.sub, Some("auth0|123".to_owned()));
    }

    #[test]
    fn auth_failure_maps_reason() {
        let event = AuditEvent::AuthFailure {
            reason: "invalid_token".to_owned(),
        };
        let proxy = AuditEventProxy::from(&event);
        assert_eq!(proxy.event_type, "auth_failure");
        assert_eq!(proxy.reason, Some("invalid_token".to_owned()));
        assert!(proxy.user_id.is_none());
    }

    #[test]
    fn role_denied_maps_roles() {
        let event = AuditEvent::RoleDenied {
            user_id: sample_uuid(),
            required_role: "manager".to_owned(),
            actual_role: "user".to_owned(),
        };
        let proxy = AuditEventProxy::from(&event);
        assert_eq!(proxy.event_type, "role_denied");
        assert_eq!(proxy.user_id, Some(sample_uuid().to_string()));
        assert_eq!(proxy.required_role, Some("manager".to_owned()));
        assert_eq!(proxy.actual_role, Some("user".to_owned()));
    }

    #[test]
    fn user_created_maps_actor_and_created() {
        let actor = sample_uuid();
        let created = uuid::Uuid::parse_str("660e8400-e29b-41d4-a716-446655440001").unwrap();
        let event = AuditEvent::UserCreated {
            actor_id: actor,
            created_id: created,
            email: "bob@example.com".to_owned(),
        };
        let proxy = AuditEventProxy::from(&event);
        assert_eq!(proxy.event_type, "user_created");
        assert_eq!(proxy.actor_id, Some(actor.to_string()));
        assert_eq!(proxy.created_id, Some(created.to_string()));
        assert_eq!(proxy.email, Some("bob@example.com".to_owned()));
    }

    #[test]
    fn user_updated_maps_actor_and_target() {
        let actor = sample_uuid();
        let target = uuid::Uuid::parse_str("660e8400-e29b-41d4-a716-446655440001").unwrap();
        let event = AuditEvent::UserUpdated {
            actor_id: actor,
            target_id: target,
        };
        let proxy = AuditEventProxy::from(&event);
        assert_eq!(proxy.event_type, "user_updated");
        assert_eq!(proxy.actor_id, Some(actor.to_string()));
        assert_eq!(proxy.target_id, Some(target.to_string()));
    }

    #[test]
    fn user_deleted_maps_actor_and_target() {
        let actor = sample_uuid();
        let target = uuid::Uuid::parse_str("660e8400-e29b-41d4-a716-446655440001").unwrap();
        let event = AuditEvent::UserDeleted {
            actor_id: actor,
            target_id: target,
        };
        let proxy = AuditEventProxy::from(&event);
        assert_eq!(proxy.event_type, "user_deleted");
        assert_eq!(proxy.actor_id, Some(actor.to_string()));
        assert_eq!(proxy.target_id, Some(target.to_string()));
    }

    #[test]
    fn user_provisioned_maps_fields() {
        let event = AuditEvent::UserProvisioned {
            user_id: sample_uuid(),
            email: "charlie@example.com".to_owned(),
            role: "manager".to_owned(),
        };
        let proxy = AuditEventProxy::from(&event);
        assert_eq!(proxy.event_type, "user_provisioned");
        assert_eq!(proxy.user_id, Some(sample_uuid().to_string()));
        assert_eq!(proxy.email, Some("charlie@example.com".to_owned()));
        assert_eq!(proxy.role, Some("manager".to_owned()));
    }
}
