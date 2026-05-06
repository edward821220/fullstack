use axum::{
    body::Body,
    http::{Request, StatusCode},
    middleware::from_fn,
};
use server::handlers::users::routes as users_routes;
use server::middleware::oidc::OidcValidator;
use server::state::AppState;
use server::{audit::NoopExporter, health_checker::AlwaysHealthy};
use std::sync::Arc;
use svc::audit::PiiMode;
use svc::{AuditService, ProvisioningPolicy, UserService, UserServiceTrait};
use tower::ServiceExt;

mod common;

fn test_state() -> Arc<AppState> {
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
        allowed_algorithms: vec!["RS256".to_owned(), "RS384".to_owned(), "RS512".to_owned()],
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
    let health_checker: Arc<dyn svc::HealthChecker> = Arc::new(AlwaysHealthy);
    Arc::new(AppState {
        svc,
        health: health_checker,
        oidc,
        provisioning,
        audit,
    })
}

#[tokio::test]
async fn users_list_should_return_200_with_paginated_response() {
    let state = test_state();
    let app = users_routes(state).layer(from_fn(
        |mut req: Request<Body>, next: axum::middleware::Next| async move {
            req.extensions_mut()
                .insert(common::make_auth_user("manager"));
            next.run(req).await
        },
    ));
    let req = Request::builder()
        .uri("/users")
        .body(Body::empty())
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = axum::body::to_bytes(res.into_body(), 1024).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(json["data"].is_array());
    assert_eq!(json["total"], 0);
    assert_eq!(json["page"], 1);
}

#[tokio::test]
async fn users_create_should_return_201_for_admin() {
    let state = test_state();
    let app = users_routes(state).layer(from_fn(
        |mut req: Request<Body>, next: axum::middleware::Next| async move {
            req.extensions_mut().insert(common::make_auth_user("admin"));
            next.run(req).await
        },
    ));
    let payload = serde_json::json!({"email":"new@example.com","display_name":"New User"});
    let req = Request::builder()
        .uri("/users")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(payload.to_string()))
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let body = axum::body::to_bytes(res.into_body(), 1024).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["email"], "new@example.com");
    assert_eq!(json["display_name"], "New User");
}

#[tokio::test]
async fn users_create_should_return_403_for_manager() {
    let state = test_state();
    let app = users_routes(state).layer(from_fn(
        |mut req: Request<Body>, next: axum::middleware::Next| async move {
            req.extensions_mut()
                .insert(common::make_auth_user("manager"));
            next.run(req).await
        },
    ));
    let payload = serde_json::json!({"email":"new@example.com","display_name":"New User"});
    let req = Request::builder()
        .uri("/users")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(payload.to_string()))
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn users_get_should_return_200_for_existing_user() {
    let state = test_state();
    let user = state
        .svc
        .create_user(
            "test@example.com",
            "Test User",
            model::role::Role::User,
            false,
        )
        .await
        .unwrap();
    let app = users_routes(state).layer(from_fn(
        |mut req: Request<Body>, next: axum::middleware::Next| async move {
            req.extensions_mut()
                .insert(common::make_auth_user("manager"));
            next.run(req).await
        },
    ));
    let req = Request::builder()
        .uri(format!("/users/{}", user.id))
        .body(Body::empty())
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = axum::body::to_bytes(res.into_body(), 1024).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["email"], "test@example.com");
}

#[tokio::test]
async fn users_update_should_return_200_for_manager() {
    let state = test_state();
    let user = state
        .svc
        .create_user(
            "test@example.com",
            "Test User",
            model::role::Role::User,
            false,
        )
        .await
        .unwrap();
    let app = users_routes(state).layer(from_fn(
        |mut req: Request<Body>, next: axum::middleware::Next| async move {
            req.extensions_mut()
                .insert(common::make_auth_user("manager"));
            next.run(req).await
        },
    ));
    let payload = serde_json::json!({"display_name":"Updated Name"});
    let req = Request::builder()
        .uri(format!("/users/{}", user.id))
        .method("PUT")
        .header("content-type", "application/json")
        .body(Body::from(payload.to_string()))
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = axum::body::to_bytes(res.into_body(), 1024).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["display_name"], "Updated Name");
}

#[tokio::test]
async fn users_delete_should_return_204_for_admin() {
    let state = test_state();
    let user = state
        .svc
        .create_user(
            "test@example.com",
            "Test User",
            model::role::Role::User,
            false,
        )
        .await
        .unwrap();
    let app = users_routes(state).layer(from_fn(
        |mut req: Request<Body>, next: axum::middleware::Next| async move {
            req.extensions_mut().insert(common::make_auth_user("admin"));
            next.run(req).await
        },
    ));
    let req = Request::builder()
        .uri(format!("/users/{}", user.id))
        .method("DELETE")
        .body(Body::empty())
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn users_delete_should_return_403_for_manager() {
    let state = test_state();
    let user = state
        .svc
        .create_user(
            "test@example.com",
            "Test User",
            model::role::Role::User,
            false,
        )
        .await
        .unwrap();
    let app = users_routes(state).layer(from_fn(
        |mut req: Request<Body>, next: axum::middleware::Next| async move {
            req.extensions_mut()
                .insert(common::make_auth_user("manager"));
            next.run(req).await
        },
    ));
    let req = Request::builder()
        .uri(format!("/users/{}", user.id))
        .method("DELETE")
        .body(Body::empty())
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
}
