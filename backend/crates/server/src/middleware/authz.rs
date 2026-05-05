use axum::{
    extract::Request,
    middleware::Next,
    response::{IntoResponse, Response},
};

use crate::audit::{AuditEvent, log_audit_event};
pub use crate::authz::domain::{AuthzError, Role, authorize_role};
use crate::middleware::oidc::AuthUser;
use crate::problem::ProblemResponse;

impl IntoResponse for AuthzError {
    fn into_response(self) -> Response {
        let AuthzError::Forbidden(detail) = self;
        ProblemResponse::forbidden(detail).into_response()
    }
}

async fn enforce_role(
    minimum_role: Role,
    req: Request,
    next: Next,
) -> Result<Response, AuthzError> {
    let auth_user = req.extensions().get::<AuthUser>();

    match auth_user {
        None => Ok(next.run(req).await),
        Some(user) => {
            if let Err(e) = authorize_role(&user.role, &minimum_role) {
                tracing::warn!(
                    user_id = %user.user_id,
                    user_role = %user.role,
                    minimum_role = %minimum_role.as_str(),
                    "Authorization denied"
                );
                log_audit_event(&AuditEvent::RoleDenied {
                    user_id: user.user_id,
                    required_role: minimum_role.as_str().to_owned(),
                    actual_role: user.role.clone(),
                });
                return Err(e);
            }

            Ok(next.run(req).await)
        }
    }
}

pub async fn require_admin(req: Request, next: Next) -> Result<Response, AuthzError> {
    enforce_role(Role::Admin, req, next).await
}

pub async fn require_manager(req: Request, next: Next) -> Result<Response, AuthzError> {
    enforce_role(Role::Manager, req, next).await
}

pub async fn require_user(req: Request, next: Next) -> Result<Response, AuthzError> {
    enforce_role(Role::User, req, next).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        Router,
        body::Body,
        http::{Request, StatusCode},
        routing::get,
    };
    use tower::ServiceExt;

    use crate::middleware::oidc::AuthUser;

    fn make_auth_user(role: &str) -> AuthUser {
        AuthUser {
            user_id: uuid::Uuid::new_v4(),
            email: "test@example.com".to_owned(),
            display_name: "Test User".to_owned(),
            role: role.to_owned(),
            sub: "test-sub".to_owned(),
        }
    }

    async fn mock_handler() -> &'static str {
        "ok"
    }

    #[tokio::test]
    async fn require_admin_should_allow_admin() {
        let app = Router::new()
            .route(
                "/test",
                get(mock_handler).layer(axum::middleware::from_fn(require_admin)),
            )
            .layer(axum::middleware::from_fn(
                |mut req: Request<Body>, next: Next| async move {
                    req.extensions_mut().insert(make_auth_user("admin"));
                    next.run(req).await
                },
            ));

        let req = Request::builder().uri("/test").body(Body::empty()).unwrap();
        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn require_admin_should_forbid_manager() {
        let app = Router::new()
            .route(
                "/test",
                get(mock_handler).layer(axum::middleware::from_fn(require_admin)),
            )
            .layer(axum::middleware::from_fn(
                |mut req: Request<Body>, next: Next| async move {
                    req.extensions_mut().insert(make_auth_user("manager"));
                    next.run(req).await
                },
            ));

        let req = Request::builder().uri("/test").body(Body::empty()).unwrap();
        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn require_admin_should_forbid_user() {
        let app = Router::new()
            .route(
                "/test",
                get(mock_handler).layer(axum::middleware::from_fn(require_admin)),
            )
            .layer(axum::middleware::from_fn(
                |mut req: Request<Body>, next: Next| async move {
                    req.extensions_mut().insert(make_auth_user("user"));
                    next.run(req).await
                },
            ));

        let req = Request::builder().uri("/test").body(Body::empty()).unwrap();
        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn require_manager_should_allow_admin() {
        let app = Router::new()
            .route(
                "/test",
                get(mock_handler).layer(axum::middleware::from_fn(require_manager)),
            )
            .layer(axum::middleware::from_fn(
                |mut req: Request<Body>, next: Next| async move {
                    req.extensions_mut().insert(make_auth_user("admin"));
                    next.run(req).await
                },
            ));

        let req = Request::builder().uri("/test").body(Body::empty()).unwrap();
        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn require_manager_should_allow_manager() {
        let app = Router::new()
            .route(
                "/test",
                get(mock_handler).layer(axum::middleware::from_fn(require_manager)),
            )
            .layer(axum::middleware::from_fn(
                |mut req: Request<Body>, next: Next| async move {
                    req.extensions_mut().insert(make_auth_user("manager"));
                    next.run(req).await
                },
            ));

        let req = Request::builder().uri("/test").body(Body::empty()).unwrap();
        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn require_manager_should_forbid_user() {
        let app = Router::new()
            .route(
                "/test",
                get(mock_handler).layer(axum::middleware::from_fn(require_manager)),
            )
            .layer(axum::middleware::from_fn(
                |mut req: Request<Body>, next: Next| async move {
                    req.extensions_mut().insert(make_auth_user("user"));
                    next.run(req).await
                },
            ));

        let req = Request::builder().uri("/test").body(Body::empty()).unwrap();
        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn authz_should_pass_through_when_no_auth_user() {
        let app = Router::new().route(
            "/test",
            get(mock_handler).layer(axum::middleware::from_fn(require_admin)),
        );

        let req = Request::builder().uri("/test").body(Body::empty()).unwrap();
        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn authz_should_reject_unknown_role() {
        let app = Router::new()
            .route(
                "/test",
                get(mock_handler).layer(axum::middleware::from_fn(require_manager)),
            )
            .layer(axum::middleware::from_fn(
                |mut req: Request<Body>, next: Next| async move {
                    req.extensions_mut().insert(make_auth_user("superadmin"));
                    next.run(req).await
                },
            ));

        let req = Request::builder().uri("/test").body(Body::empty()).unwrap();
        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::FORBIDDEN);
    }
}
