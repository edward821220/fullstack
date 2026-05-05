use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
    middleware::from_fn,
    routing::get,
};
use tower::ServiceExt;

mod common;

#[tokio::test]
async fn authz_should_return_403_problem_details_when_forbidden() {
    let app = Router::new()
        .route("/test", get(|| async { "ok" }))
        .route_layer(from_fn(server::middleware::require_admin))
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
async fn authz_should_pass_through_when_no_auth_user() {
    let app = Router::new()
        .route("/test", get(|| async { "ok" }))
        .route_layer(from_fn(server::middleware::require_admin));

    let req = Request::builder().uri("/test").body(Body::empty()).unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn admin_should_pass_require_admin() {
    let app = Router::new()
        .route("/test", get(|| async { "ok" }))
        .route_layer(from_fn(server::middleware::require_admin))
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
    let app = Router::new()
        .route("/test", get(|| async { "ok" }))
        .route_layer(from_fn(server::middleware::require_manager))
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
    let app = Router::new()
        .route("/test", get(|| async { "ok" }))
        .route_layer(from_fn(server::middleware::require_admin))
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
    let app = Router::new()
        .route("/test", get(|| async { "ok" }))
        .route_layer(from_fn(server::middleware::require_manager))
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
    let app = Router::new()
        .route("/test", get(|| async { "ok" }))
        .route_layer(from_fn(server::middleware::require_manager))
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
    let app = Router::new()
        .route("/test", get(|| async { "ok" }))
        .route_layer(from_fn(server::middleware::require_admin))
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
        .route_layer(from_fn(server::middleware::require_manager))
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
