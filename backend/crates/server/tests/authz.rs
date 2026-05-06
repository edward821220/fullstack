use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
    middleware::{from_fn, from_fn_with_state},
    routing::get,
};
use std::sync::Arc;
use svc::audit::PiiMode;
use tower::ServiceExt;

mod common;

fn mock_state() -> Arc<server::state::AppState> {
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
    let oidc = Arc::new(server::middleware::oidc::OidcValidator::new(cfg));
    let svc = Arc::new(svc::UserService::new(repo::AnyUserRepo::Mock(
        repo::MockUserRepo::new(),
    )));
    let provisioning = svc::ProvisioningPolicy::new(vec![], "user".to_owned());
    let audit_exporter: Arc<dyn svc::AuditExporter> = Arc::new(infra::audit::NoopExporter);
    let audit = svc::AuditService::new(audit_exporter, PiiMode::Full);
    Arc::new(server::state::AppState {
        svc,
        health: Arc::new(infra::health_checker::AlwaysHealthy),
        oidc,
        provisioning,
        audit,
    })
}

#[tokio::test]
async fn authz_should_return_403_problem_details_when_forbidden() {
    let state = mock_state();
    let app = Router::new()
        .route("/test", get(|| async { "ok" }))
        .route_layer(from_fn_with_state(
            state.clone(),
            server::middleware::require_admin,
        ))
        .layer(from_fn(
            |mut req: Request<Body>, next: axum::middleware::Next| async move {
                req.extensions_mut().insert(common::make_auth_user("user"));
                next.run(req).await
            },
        ));

    let req = Request::builder().uri("/test").body(Body::empty()).unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    let body = axum::body::to_bytes(res.into_body(), 1024).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["status"], 403);
    assert_eq!(json["type"], "about:blank");
    assert!(json["detail"].as_str().unwrap().contains("not authorized"));
}

#[tokio::test]
async fn authz_should_pass_through_when_auth_disabled() {
    let state = mock_state();
    let app = Router::new()
        .route("/test", get(|| async { "ok" }))
        .route_layer(from_fn_with_state(
            state.clone(),
            server::middleware::require_admin,
        ))
        .layer(from_fn(
            |mut req: Request<Body>, next: axum::middleware::Next| async move {
                req.extensions_mut()
                    .insert(server::middleware::oidc::AuthDisabledMarker);
                next.run(req).await
            },
        ));

    let req = Request::builder().uri("/test").body(Body::empty()).unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn authz_should_forbid_when_auth_enabled_and_no_user() {
    let state = mock_state();
    let app = Router::new()
        .route("/test", get(|| async { "ok" }))
        .route_layer(from_fn_with_state(
            state.clone(),
            server::middleware::require_admin,
        ));

    let req = Request::builder().uri("/test").body(Body::empty()).unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn admin_should_pass_require_admin() {
    let state = mock_state();
    let app = Router::new()
        .route("/test", get(|| async { "ok" }))
        .route_layer(from_fn_with_state(
            state.clone(),
            server::middleware::require_admin,
        ))
        .layer(from_fn(
            |mut req: Request<Body>, next: axum::middleware::Next| async move {
                req.extensions_mut().insert(common::make_auth_user("admin"));
                next.run(req).await
            },
        ));

    let req = Request::builder().uri("/test").body(Body::empty()).unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn manager_should_pass_require_manager() {
    let state = mock_state();
    let app = Router::new()
        .route("/test", get(|| async { "ok" }))
        .route_layer(from_fn_with_state(
            state.clone(),
            server::middleware::require_manager,
        ))
        .layer(from_fn(
            |mut req: Request<Body>, next: axum::middleware::Next| async move {
                req.extensions_mut()
                    .insert(common::make_auth_user("manager"));
                next.run(req).await
            },
        ));

    let req = Request::builder().uri("/test").body(Body::empty()).unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn user_should_be_forbidden_by_require_admin() {
    let state = mock_state();
    let app = Router::new()
        .route("/test", get(|| async { "ok" }))
        .route_layer(from_fn_with_state(
            state.clone(),
            server::middleware::require_admin,
        ))
        .layer(from_fn(
            |mut req: Request<Body>, next: axum::middleware::Next| async move {
                req.extensions_mut().insert(common::make_auth_user("user"));
                next.run(req).await
            },
        ));

    let req = Request::builder().uri("/test").body(Body::empty()).unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn user_should_be_forbidden_by_require_manager() {
    let state = mock_state();
    let app = Router::new()
        .route("/test", get(|| async { "ok" }))
        .route_layer(from_fn_with_state(
            state.clone(),
            server::middleware::require_manager,
        ))
        .layer(from_fn(
            |mut req: Request<Body>, next: axum::middleware::Next| async move {
                req.extensions_mut().insert(common::make_auth_user("user"));
                next.run(req).await
            },
        ));

    let req = Request::builder().uri("/test").body(Body::empty()).unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn unknown_role_should_be_forbidden() {
    let state = mock_state();
    let app = Router::new()
        .route("/test", get(|| async { "ok" }))
        .route_layer(from_fn_with_state(
            state.clone(),
            server::middleware::require_manager,
        ))
        .layer(from_fn(
            |mut req: Request<Body>, next: axum::middleware::Next| async move {
                req.extensions_mut()
                    .insert(common::make_auth_user("superadmin"));
                next.run(req).await
            },
        ));

    let req = Request::builder().uri("/test").body(Body::empty()).unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn admin_should_pass_require_admin_and_manager() {
    let state = mock_state();
    let app = Router::new()
        .route("/test", get(|| async { "ok" }))
        .route_layer(from_fn_with_state(
            state.clone(),
            server::middleware::require_admin,
        ))
        .layer(from_fn(
            |mut req: Request<Body>, next: axum::middleware::Next| async move {
                req.extensions_mut().insert(common::make_auth_user("admin"));
                next.run(req).await
            },
        ));

    let req = Request::builder().uri("/test").body(Body::empty()).unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let app = Router::new()
        .route("/test", get(|| async { "ok" }))
        .route_layer(from_fn_with_state(
            state.clone(),
            server::middleware::require_manager,
        ))
        .layer(from_fn(
            |mut req: Request<Body>, next: axum::middleware::Next| async move {
                req.extensions_mut().insert(common::make_auth_user("admin"));
                next.run(req).await
            },
        ));

    let req = Request::builder().uri("/test").body(Body::empty()).unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}
