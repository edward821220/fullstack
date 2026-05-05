use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
    routing::get,
};
use server::problem::ProblemResponse;
use tower::ServiceExt;

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
