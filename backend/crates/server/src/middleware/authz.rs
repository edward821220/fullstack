use std::str::FromStr;

use axum::{extract::Request, middleware::Next, response::Response};

use super::oidc::AuthUser;
use crate::problem::ProblemResponse;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Role {
    Admin,
    Manager,
    User,
}

impl FromStr for Role {
    type Err = ();

    fn from_str(role: &str) -> Result<Self, Self::Err> {
        match role {
            "admin" => Ok(Role::Admin),
            "manager" => Ok(Role::Manager),
            "user" => Ok(Role::User),
            _ => Err(()),
        }
    }
}

impl Role {
    pub fn as_str(&self) -> &'static str {
        match self {
            Role::Admin => "admin",
            Role::Manager => "manager",
            Role::User => "user",
        }
    }
}

fn has_permission(user_role: &Role, required_role: &Role) -> bool {
    matches!(
        (user_role, required_role),
        (Role::Admin, _) | (Role::Manager, Role::Manager | Role::User) | (Role::User, Role::User)
    )
}

async fn enforce_role(minimum_role: Role, req: Request, next: Next) -> Result<Response, Response> {
    let auth_user = req.extensions().get::<AuthUser>();

    match auth_user {
        None => Ok(next.run(req).await),
        Some(user) => {
            let user_role = Role::from_str(&user.role).map_err(|_| {
                tracing::warn!(role = %user.role, "Unknown user role");
                Response::from(ProblemResponse::forbidden(format!(
                    "The role '{}' assigned to your identity is not recognized by this system",
                    user.role
                )))
            })?;

            if !has_permission(&user_role, &minimum_role) {
                tracing::warn!(
                    user_id = %user.user_id,
                    user_role = %user_role.as_str(),
                    minimum_role = %minimum_role.as_str(),
                    "Authorization denied"
                );
                return Err(Response::from(ProblemResponse::forbidden(format!(
                    "Role '{}' is not authorized for this operation (requires '{}' or higher)",
                    user_role.as_str(),
                    minimum_role.as_str()
                ))));
            }

            Ok(next.run(req).await)
        }
    }
}

pub async fn require_admin(req: Request, next: Next) -> Result<Response, Response> {
    enforce_role(Role::Admin, req, next).await
}

pub async fn require_manager(req: Request, next: Next) -> Result<Response, Response> {
    enforce_role(Role::Manager, req, next).await
}

pub async fn require_user(req: Request, next: Next) -> Result<Response, Response> {
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

    #[test]
    fn admin_can_access_all_roles() {
        assert!(has_permission(&Role::Admin, &Role::Admin));
        assert!(has_permission(&Role::Admin, &Role::Manager));
        assert!(has_permission(&Role::Admin, &Role::User));
    }

    #[test]
    fn manager_can_access_manager_and_user() {
        assert!(has_permission(&Role::Manager, &Role::Manager));
        assert!(has_permission(&Role::Manager, &Role::User));
        assert!(!has_permission(&Role::Manager, &Role::Admin));
    }

    #[test]
    fn user_can_only_access_user() {
        assert!(has_permission(&Role::User, &Role::User));
        assert!(!has_permission(&Role::User, &Role::Manager));
        assert!(!has_permission(&Role::User, &Role::Admin));
    }

    #[test]
    fn role_from_str_should_parse_known_roles() {
        assert_eq!(Role::from_str("admin"), Ok(Role::Admin));
        assert_eq!(Role::from_str("manager"), Ok(Role::Manager));
        assert_eq!(Role::from_str("user"), Ok(Role::User));
        assert_eq!(Role::from_str("unknown"), Err(()));
    }
}
