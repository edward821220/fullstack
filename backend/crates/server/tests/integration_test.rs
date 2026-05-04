use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
    middleware::from_fn,
    routing::get,
};
use tower::ServiceExt;

use server::middleware::{
    authz::{require_admin, require_manager},
    oidc::AuthUser,
};
use uuid::Uuid;

fn make_auth_user(role: &str) -> AuthUser {
    AuthUser {
        user_id: Uuid::new_v4(),
        email: format!("{role}@example.com"),
        display_name: format!("{role}-user"),
        role: role.to_owned(),
        sub: format!("sub-{role}"),
    }
}

// ── Health endpoint tests ───────────────────────────────────────────────────

#[tokio::test]
async fn health_should_return_200_with_status_ok() {
    let app = Router::new().route("/health", get(server::handlers::health::health));

    let req = Request::builder()
        .uri("/health")
        .body(Body::empty())
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let body = axum::body::to_bytes(res.into_body(), 1024).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["status"], "ok");
    assert!(!json["version"].as_str().unwrap().is_empty());
}

// ── Authz middleware integration tests ──────────────────────────────────────

#[tokio::test]
async fn authz_should_return_403_problem_details_when_forbidden() {
    let app = Router::new()
        .route("/test", get(|| async { "ok" }))
        .route_layer(from_fn(require_admin))
        .layer(from_fn(
            |mut req: Request<Body>, next: axum::middleware::Next| async move {
                req.extensions_mut().insert(make_auth_user("user"));
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
async fn authz_should_pass_through_when_no_auth_user() {
    let app = Router::new()
        .route("/test", get(|| async { "ok" }))
        .route_layer(from_fn(require_admin));

    let req = Request::builder().uri("/test").body(Body::empty()).unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn admin_should_pass_require_admin() {
    let app = Router::new()
        .route("/test", get(|| async { "ok" }))
        .route_layer(from_fn(require_admin))
        .layer(from_fn(
            |mut req: Request<Body>, next: axum::middleware::Next| async move {
                req.extensions_mut().insert(make_auth_user("admin"));
                next.run(req).await
            },
        ));

    let req = Request::builder().uri("/test").body(Body::empty()).unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn manager_should_pass_require_manager() {
    let app = Router::new()
        .route("/test", get(|| async { "ok" }))
        .route_layer(from_fn(require_manager))
        .layer(from_fn(
            |mut req: Request<Body>, next: axum::middleware::Next| async move {
                req.extensions_mut().insert(make_auth_user("manager"));
                next.run(req).await
            },
        ));

    let req = Request::builder().uri("/test").body(Body::empty()).unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn user_should_be_forbidden_by_require_admin() {
    let app = Router::new()
        .route("/test", get(|| async { "ok" }))
        .route_layer(from_fn(require_admin))
        .layer(from_fn(
            |mut req: Request<Body>, next: axum::middleware::Next| async move {
                req.extensions_mut().insert(make_auth_user("user"));
                next.run(req).await
            },
        ));

    let req = Request::builder().uri("/test").body(Body::empty()).unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn user_should_be_forbidden_by_require_manager() {
    let app = Router::new()
        .route("/test", get(|| async { "ok" }))
        .route_layer(from_fn(require_manager))
        .layer(from_fn(
            |mut req: Request<Body>, next: axum::middleware::Next| async move {
                req.extensions_mut().insert(make_auth_user("user"));
                next.run(req).await
            },
        ));

    let req = Request::builder().uri("/test").body(Body::empty()).unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn unknown_role_should_be_forbidden() {
    let app = Router::new()
        .route("/test", get(|| async { "ok" }))
        .route_layer(from_fn(require_manager))
        .layer(from_fn(
            |mut req: Request<Body>, next: axum::middleware::Next| async move {
                req.extensions_mut().insert(make_auth_user("superadmin"));
                next.run(req).await
            },
        ));

    let req = Request::builder().uri("/test").body(Body::empty()).unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn admin_should_pass_require_admin_and_manager() {
    // Admin passes require_admin
    let app = Router::new()
        .route("/test", get(|| async { "ok" }))
        .route_layer(from_fn(require_admin))
        .layer(from_fn(
            |mut req: Request<Body>, next: axum::middleware::Next| async move {
                req.extensions_mut().insert(make_auth_user("admin"));
                next.run(req).await
            },
        ));

    let req = Request::builder().uri("/test").body(Body::empty()).unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // Admin passes require_manager
    let app = Router::new()
        .route("/test", get(|| async { "ok" }))
        .route_layer(from_fn(require_manager))
        .layer(from_fn(
            |mut req: Request<Body>, next: axum::middleware::Next| async move {
                req.extensions_mut().insert(make_auth_user("admin"));
                next.run(req).await
            },
        ));

    let req = Request::builder().uri("/test").body(Body::empty()).unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

// ── ProblemResponse integration tests ────────────────────────────────────────

use server::problem::ProblemResponse;

#[tokio::test]
async fn unauthorised_should_return_401_with_problem_details() {
    let app = Router::new().route(
        "/unauth",
        get(|| async {
            axum::response::Response::from(ProblemResponse::unauthorized("Invalid token"))
        }),
    );

    let req = Request::builder()
        .uri("/unauth")
        .body(Body::empty())
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    let body = axum::body::to_bytes(res.into_body(), 1024).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["status"], 401);
    assert_eq!(json["type"], "about:blank");
    assert_eq!(json["title"], "Unauthorized");
}

#[tokio::test]
async fn internal_error_should_return_500_with_problem_details() {
    let app = Router::new().route(
        "/error",
        get(|| async {
            axum::response::Response::from(ProblemResponse::internal_error("Something broke"))
        }),
    );

    let req = Request::builder()
        .uri("/error")
        .body(Body::empty())
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);

    let body = axum::body::to_bytes(res.into_body(), 1024).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["status"], 500);
    assert_eq!(json["type"], "about:blank");
}

#[tokio::test]
async fn service_unavailable_should_return_503_with_problem_details() {
    let app = Router::new().route(
        "/unavailable",
        get(|| async {
            axum::response::Response::from(ProblemResponse::service_unavailable("DB unreachable"))
        }),
    );

    let req = Request::builder()
        .uri("/unavailable")
        .body(Body::empty())
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::SERVICE_UNAVAILABLE);

    let body = axum::body::to_bytes(res.into_body(), 1024).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["status"], 503);
}
