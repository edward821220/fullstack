use super::{AuthzError, Role, authorize_role};
use crate::middleware::oidc::{AuthDisabledMarker, AuthUser};
use crate::state::AppState;
use axum::{
    extract::{Request, State},
    middleware::Next,
    response::Response,
};
use std::sync::Arc;
use svc::AuditEvent;

async fn enforce_role(
    State(state): State<Arc<AppState>>,
    minimum_role: Role,
    req: Request,
    next: Next,
) -> Result<Response, AuthzError> {
    let auth_user = req.extensions().get::<AuthUser>();
    let auth_disabled = req.extensions().get::<AuthDisabledMarker>().is_some();
    match auth_user {
        None if auth_disabled => Ok(next.run(req).await),
        None => {
            tracing::warn!("Authorization denied: no authenticated user (auth_enabled=true)");
            Err(AuthzError::Forbidden(
                "Authentication required for this resource".to_owned(),
            ))
        }
        Some(user) => {
            if let Err(e) = authorize_role(&user.role, &minimum_role) {
                tracing::warn!(
                    user_id = %user.user_id,
                    user_role = %user.role,
                    minimum_role = %minimum_role.as_str(),
                    "Authorization denied"
                );
                state.audit.record(AuditEvent::RoleDenied {
                    user_id: user.user_id,
                    required_role: minimum_role.as_str().to_owned(),
                    actual_role: user.role.to_string(),
                });
                return Err(e);
            }
            Ok(next.run(req).await)
        }
    }
}

pub async fn require_admin(
    state: State<Arc<AppState>>,
    req: Request,
    next: Next,
) -> Result<Response, AuthzError> {
    enforce_role(state, Role::Admin, req, next).await
}

pub async fn require_manager(
    state: State<Arc<AppState>>,
    req: Request,
    next: Next,
) -> Result<Response, AuthzError> {
    enforce_role(state, Role::Manager, req, next).await
}

pub async fn require_user(
    state: State<Arc<AppState>>,
    req: Request,
    next: Next,
) -> Result<Response, AuthzError> {
    enforce_role(state, Role::User, req, next).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::NoopExporter;
    use crate::middleware::oidc::{AuthUser, OidcValidator};
    use crate::state::AppState;
    use axum::{
        Router,
        body::Body,
        http::{Request, StatusCode},
        middleware::{from_fn, from_fn_with_state},
        routing::get,
    };
    use std::str::FromStr;
    use svc::audit::PiiMode;
    use svc::{AuditService, ProvisioningPolicy, UserService};
    use tower::ServiceExt;

    fn mock_state() -> Arc<AppState> {
        let cfg = config::AuthConfig {
            enabled: false,
            issuer_url: "http://localhost:8080".to_owned(),
            audience: vec!["test".to_owned()],
            jwks_cache_duration_secs: 3600,
            allowed_email_domains: vec![],
            allow_all_domains: false,
            role_claim_source: config::RoleClaimSource::Roles,
            discovery_mode: config::DiscoveryMode::Discovery,
            manual_endpoints: None,
            danger_accept_invalid_certs: false,
            allowed_algorithms: vec!["RS256".to_owned()],
            require_email_verified: false,
            clock_skew_seconds: 60,
        };
        let oidc = Arc::new(OidcValidator::new(cfg));
        let svc = Arc::new(UserService::new(repo::AnyUserRepo::Mock(
            repo::MockUserRepo::new(),
        )));
        let provisioning = ProvisioningPolicy::new(vec![], "user".to_owned());
        let audit_exporter: Arc<dyn svc::AuditExporter> = Arc::new(NoopExporter);
        let audit = AuditService::new(audit_exporter, PiiMode::Full);
        Arc::new(AppState {
            svc,
            health: Arc::new(crate::health_checker::AlwaysHealthy),
            oidc,
            provisioning,
            audit,
        })
    }

    fn make_auth_user(role: model::role::Role) -> AuthUser {
        AuthUser {
            user_id: uuid::Uuid::new_v4(),
            email: "test@example.com".to_owned(),
            display_name: "Test User".to_owned(),
            role,
            sub: "test-sub".to_owned(),
        }
    }

    async fn mock_handler() -> &'static str {
        "ok"
    }

    #[tokio::test]
    async fn require_admin_should_allow_admin() {
        let state = mock_state();
        let app = Router::new()
            .route(
                "/test",
                get(mock_handler).layer(axum::middleware::from_fn_with_state(
                    state.clone(),
                    require_admin,
                )),
            )
            .with_state(state)
            .layer(axum::middleware::from_fn(
                |mut req: Request<Body>, next: Next| async move {
                    req.extensions_mut()
                        .insert(make_auth_user(model::role::Role::Admin));
                    next.run(req).await
                },
            ));

        let req = Request::builder().uri("/test").body(Body::empty()).unwrap();
        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn require_admin_should_forbid_manager() {
        let state = mock_state();
        let app = Router::new()
            .route(
                "/test",
                get(mock_handler).layer(axum::middleware::from_fn_with_state(
                    state.clone(),
                    require_admin,
                )),
            )
            .with_state(state)
            .layer(axum::middleware::from_fn(
                |mut req: Request<Body>, next: Next| async move {
                    req.extensions_mut()
                        .insert(make_auth_user(model::role::Role::Manager));
                    next.run(req).await
                },
            ));

        let req = Request::builder().uri("/test").body(Body::empty()).unwrap();
        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn require_admin_should_forbid_user() {
        let state = mock_state();
        let app = Router::new()
            .route(
                "/test",
                get(mock_handler).layer(axum::middleware::from_fn_with_state(
                    state.clone(),
                    require_admin,
                )),
            )
            .with_state(state)
            .layer(axum::middleware::from_fn(
                |mut req: Request<Body>, next: Next| async move {
                    req.extensions_mut()
                        .insert(make_auth_user(model::role::Role::User));
                    next.run(req).await
                },
            ));

        let req = Request::builder().uri("/test").body(Body::empty()).unwrap();
        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn require_manager_should_allow_admin() {
        let state = mock_state();
        let app = Router::new()
            .route(
                "/test",
                get(mock_handler).layer(axum::middleware::from_fn_with_state(
                    state.clone(),
                    require_manager,
                )),
            )
            .with_state(state)
            .layer(axum::middleware::from_fn(
                |mut req: Request<Body>, next: Next| async move {
                    req.extensions_mut()
                        .insert(make_auth_user(model::role::Role::Admin));
                    next.run(req).await
                },
            ));

        let req = Request::builder().uri("/test").body(Body::empty()).unwrap();
        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn require_manager_should_allow_manager() {
        let state = mock_state();
        let app = Router::new()
            .route(
                "/test",
                get(mock_handler).layer(axum::middleware::from_fn_with_state(
                    state.clone(),
                    require_manager,
                )),
            )
            .with_state(state)
            .layer(axum::middleware::from_fn(
                |mut req: Request<Body>, next: Next| async move {
                    req.extensions_mut()
                        .insert(make_auth_user(model::role::Role::Manager));
                    next.run(req).await
                },
            ));

        let req = Request::builder().uri("/test").body(Body::empty()).unwrap();
        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn require_manager_should_forbid_user() {
        let state = mock_state();
        let app = Router::new()
            .route(
                "/test",
                get(mock_handler).layer(axum::middleware::from_fn_with_state(
                    state.clone(),
                    require_manager,
                )),
            )
            .with_state(state)
            .layer(axum::middleware::from_fn(
                |mut req: Request<Body>, next: Next| async move {
                    req.extensions_mut()
                        .insert(make_auth_user(model::role::Role::User));
                    next.run(req).await
                },
            ));

        let req = Request::builder().uri("/test").body(Body::empty()).unwrap();
        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn authz_should_pass_through_when_auth_disabled() {
        let state = mock_state();
        let app = Router::new()
            .route(
                "/test",
                get(mock_handler).layer(from_fn_with_state(state.clone(), require_admin)),
            )
            .with_state(state)
            .layer(from_fn(|mut req: Request<Body>, next: Next| async move {
                req.extensions_mut().insert(AuthDisabledMarker);
                next.run(req).await
            }));

        let req = Request::builder().uri("/test").body(Body::empty()).unwrap();
        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn authz_should_forbid_when_auth_enabled_and_no_user() {
        let state = mock_state();
        let app = Router::new()
            .route(
                "/test",
                get(mock_handler).layer(from_fn_with_state(state.clone(), require_admin)),
            )
            .with_state(state);

        let req = Request::builder().uri("/test").body(Body::empty()).unwrap();
        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn authz_should_reject_unknown_role() {
        let state = mock_state();
        let app = Router::new()
            .route(
                "/test",
                get(mock_handler).layer(axum::middleware::from_fn_with_state(
                    state.clone(),
                    require_manager,
                )),
            )
            .with_state(state)
            .layer(axum::middleware::from_fn(
                |mut req: Request<Body>, next: Next| async move {
                    req.extensions_mut().insert(make_auth_user(
                        model::role::Role::from_str("superadmin")
                            .unwrap_or(model::role::Role::User),
                    ));
                    next.run(req).await
                },
            ));

        let req = Request::builder().uri("/test").body(Body::empty()).unwrap();
        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::FORBIDDEN);
    }
}
