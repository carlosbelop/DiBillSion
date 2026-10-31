//! `POST /v1/receipts/scan` against a fake OpenAI-compatible provider.

use std::time::Duration;

use axum::Json;
use axum::http::{Method, StatusCode};
use axum::routing::post;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use dibill_receipt::ProviderConfig;
use dibill_receipt::openai::OutputMode;
use serde_json::{Value, json};

use super::TestDb;
use crate::Receipts;

const GOOD: &str = r#"{"merchant":"Shop","date":"2024-02-12","currency":"EUR","items":[{"description":"A","quantity":"1","unit_price":null,"total":"1.00"},{"description":"B","quantity":"2","unit_price":"1.25","total":"2.50"}],"total":"3.50"}"#;

/// Answers by image: "fail" → HTTP 500, "blur" → prose, anything else → [`GOOD`].
async fn provider(Json(body): Json<Value>) -> (StatusCode, Json<Value>) {
    let url = body["messages"][1]["content"][1]["image_url"]["url"]
        .as_str()
        .unwrap_or_default();
    let content = if url.ends_with(&STANDARD.encode("fail")) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": { "message": "down" } })),
        );
    } else if url.ends_with(&STANDARD.encode("blur")) {
        "I can't read this."
    } else {
        GOOD
    };
    (
        StatusCode::OK,
        Json(json!({ "choices": [{ "message": { "content": content } }],
                     "usage": { "prompt_tokens": 900, "completion_tokens": 90 } })),
    )
}

async fn fake_provider(daily: u32, monthly: u32) -> Receipts {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = axum::Router::new().route("/v1/chat/completions", post(provider));
    tokio::spawn(async move { axum::serve(listener, app).await });
    Receipts::new(
        ProviderConfig {
            base_url: format!("http://{addr}/v1"),
            api_key: "key".into(),
            model: "fake-vision".into(),
            mode: OutputMode::JsonSchema,
            timeout: Duration::from_secs(10),
        },
        daily,
        monthly,
    )
    .unwrap()
}

fn scan(image: &str) -> Value {
    json!({ "image_base64": STANDARD.encode(image), "mime": "image/jpeg", "currency": "EUR" })
}

#[tokio::test]
async fn disabled_without_a_provider() {
    let Some(db) = TestDb::new().await else {
        return;
    };
    let (_, token) = db.device().await;
    let (status, body) = db
        .call(
            Method::POST,
            "/v1/receipts/scan",
            Some(&token),
            Some(scan("photo")),
        )
        .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["error"], "receipts_disabled");
    db.finish().await;
}

#[tokio::test]
async fn scans_a_receipt_and_counts_it() {
    let Some(db) = TestDb::with(fake_provider(20, 1000).await).await else {
        return;
    };
    let (device, token) = db.device().await;
    let (status, _) = db
        .call(Method::POST, "/v1/receipts/scan", None, Some(scan("photo")))
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, body) = db
        .call(
            Method::POST,
            "/v1/receipts/scan",
            Some(&token),
            Some(scan("photo")),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["receipt"]["total"], "3.50");
    assert_eq!(body["receipt"]["items"].as_array().map(Vec::len), Some(2));
    assert_eq!(body["scans_left_today"], 19);

    let row = db
        .pool
        .get()
        .await
        .unwrap()
        .query_one(
            "SELECT model, ok, requests, input_tokens FROM receipt_scans WHERE device_id = $1",
            &[&device],
        )
        .await
        .unwrap();
    assert_eq!(row.get::<_, String>(0), "fake-vision");
    assert_eq!(row.get::<_, Option<bool>>(1), Some(true));
    assert_eq!(row.get::<_, Option<i16>>(2), Some(1));
    assert_eq!(row.get::<_, Option<i64>>(3), Some(900));
    db.finish().await;
}

#[tokio::test]
async fn rejects_bad_requests() {
    let Some(db) = TestDb::with(fake_provider(20, 1000).await).await else {
        return;
    };
    let (_, token) = db.device().await;
    let cases = [
        (
            json!({ "image_base64": "%%%", "mime": "image/jpeg", "currency": "EUR" }),
            StatusCode::BAD_REQUEST,
        ),
        (
            json!({ "image_base64": "", "mime": "image/jpeg", "currency": "EUR" }),
            StatusCode::BAD_REQUEST,
        ),
        (
            json!({ "image_base64": "AA==", "mime": "image/gif", "currency": "EUR" }),
            StatusCode::BAD_REQUEST,
        ),
        (
            json!({ "image_base64": "AA==", "mime": "image/jpeg", "currency": "XYZ" }),
            StatusCode::BAD_REQUEST,
        ),
        (
            json!({ "image_base64": STANDARD.encode(vec![0_u8; dibill_proto::MAX_RECEIPT_BYTES + 1]),
                    "mime": "image/jpeg", "currency": "EUR" }),
            StatusCode::PAYLOAD_TOO_LARGE,
        ),
    ];
    for (body, expected) in cases {
        let (status, answer) = db
            .call(Method::POST, "/v1/receipts/scan", Some(&token), Some(body))
            .await;
        assert_eq!(status, expected, "{answer}");
    }
    db.finish().await;
}

#[tokio::test]
async fn provider_failures_and_unreadable_images() {
    let Some(db) = TestDb::with(fake_provider(20, 1000).await).await else {
        return;
    };
    let (_, token) = db.device().await;
    let (status, body) = db
        .call(
            Method::POST,
            "/v1/receipts/scan",
            Some(&token),
            Some(scan("fail")),
        )
        .await;
    assert_eq!(
        (status, &body["error"]),
        (StatusCode::BAD_GATEWAY, &json!("provider_failed"))
    );
    let (status, body) = db
        .call(
            Method::POST,
            "/v1/receipts/scan",
            Some(&token),
            Some(scan("blur")),
        )
        .await;
    assert_eq!(
        (status, &body["error"]),
        (StatusCode::UNPROCESSABLE_ENTITY, &json!("unreadable"))
    );
    db.finish().await;
}

#[tokio::test]
async fn daily_and_monthly_limits() {
    let Some(db) = TestDb::with(fake_provider(2, 3).await).await else {
        return;
    };
    let (_, first) = db.device().await;
    for left in [1, 0] {
        let (status, body) = db
            .call(
                Method::POST,
                "/v1/receipts/scan",
                Some(&first),
                Some(scan("photo")),
            )
            .await;
        assert_eq!(
            (status, &body["scans_left_today"]),
            (StatusCode::OK, &json!(left))
        );
    }
    let (status, body) = db
        .call(
            Method::POST,
            "/v1/receipts/scan",
            Some(&first),
            Some(scan("photo")),
        )
        .await;
    assert_eq!(
        (status, &body["error"]),
        (StatusCode::TOO_MANY_REQUESTS, &json!("limit_reached"))
    );

    // A second device has its own day, but the server's month is shared: 3 in total.
    let (_, second) = db.device().await;
    let (status, _) = db
        .call(
            Method::POST,
            "/v1/receipts/scan",
            Some(&second),
            Some(scan("photo")),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = db
        .call(
            Method::POST,
            "/v1/receipts/scan",
            Some(&second),
            Some(scan("photo")),
        )
        .await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    db.finish().await;
}
