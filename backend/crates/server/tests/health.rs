use axum::{Router, body::Body, http::Request, routing::get};
use tower::ServiceExt;

#[tokio::test]
async fn health_should_return_200_with_status_ok() {
    let app = Router::new().route("/health", get(server::handlers::health::health));

    let req = Request::builder()
        .uri("/health")
        .body(Body::empty())
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), axum::http::StatusCode::OK);

    let body = axum::body::to_bytes(res.into_body(), 1024).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["status"], "ok");
    assert!(
        json.get("version").is_none(),
        "version must not be exposed in health response"
    );
}
