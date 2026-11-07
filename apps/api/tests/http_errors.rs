//! Every error response uses the `{ "error", "message" }` body, including the
//! ones produced around the handlers (wrong method, undecodable path).

mod common;

use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use common::{TestResult, app, get, send, unreachable_pool};

#[tokio::test]
async fn a_wrong_method_answers_405_json_with_allow() -> TestResult {
    let request = Request::post("/v1/info").body(Body::empty())?;
    let (status, headers, body) = send(app(unreachable_pool()?)?, request).await?;
    assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(body["error"], "method_not_allowed");
    assert!(body["message"].is_string());
    assert!(headers.get(header::ALLOW).is_some(), "Allow is kept");
    assert_eq!(headers[header::CONTENT_TYPE], "application/json");
    Ok(())
}

#[tokio::test]
async fn an_undecodable_path_answers_400_json() -> TestResult {
    let (status, headers, body) = send(app(unreachable_pool()?)?, get("/v1/users/%FF")?).await?;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"], "invalid_request");
    assert_eq!(headers[header::CONTENT_TYPE], "application/json");
    assert!(
        !body["message"]
            .as_str()
            .unwrap_or_default()
            .contains("UTF-8"),
        "framework text is not exposed"
    );
    Ok(())
}

#[tokio::test]
async fn api_errors_keep_their_own_codes() -> TestResult {
    let (status, _, body) = send(app(unreachable_pool()?)?, get("/v1/nothing-here")?).await?;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"], "not_found");
    let (status, _, body) =
        send(app(unreachable_pool()?)?, get("/v1/users/not-an-account")?).await?;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"], "invalid_account");
    Ok(())
}
