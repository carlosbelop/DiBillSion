//! The HTTP client against a fake OpenAI-compatible provider: request shape, the repair retry and
//! provider errors.
#![cfg(feature = "http")]

use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::post;
use axum::{Json, Router};
use dibill_domain::Currency;
use dibill_receipt::openai::OutputMode;
use dibill_receipt::{ProviderConfig, ScanError, Scanner, check};
use serde_json::{Value, json};

type Requests = Arc<Mutex<Vec<(Option<String>, Value)>>>;

#[derive(Clone, Default)]
struct Fake {
    answers: Arc<Mutex<Vec<(u16, String)>>>,
    requests: Requests,
}

async fn complete(
    State(fake): State<Fake>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> (StatusCode, Json<Value>) {
    let auth = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    fake.requests.lock().expect("lock").push((auth, body));
    let (status, content) = fake.answers.lock().expect("lock").remove(0);
    let response = if status == 200 {
        json!({ "choices": [{ "message": { "role": "assistant", "content": content } }],
                "usage": { "prompt_tokens": 1000, "completion_tokens": 100 } })
    } else {
        json!({ "error": { "message": content } })
    };
    (
        StatusCode::from_u16(status).expect("status"),
        Json(response),
    )
}

async fn serve(answers: Vec<(u16, &str)>) -> (Scanner, Fake) {
    let fake = Fake {
        answers: Arc::new(Mutex::new(
            answers
                .into_iter()
                .map(|(s, c)| (s, c.to_owned()))
                .collect(),
        )),
        ..Fake::default()
    };
    let app = Router::new()
        .route("/v1/chat/completions", post(complete))
        .with_state(fake.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move { axum::serve(listener, app).await });
    let scanner = Scanner::new(ProviderConfig {
        base_url: format!("http://{addr}/v1/"),
        api_key: "test-key".into(),
        model: "vision-model".into(),
        mode: OutputMode::JsonSchema,
        timeout: Duration::from_secs(10),
    })
    .expect("scanner");
    (scanner, fake)
}

fn eur() -> Currency {
    Currency::from_code("EUR").expect("EUR")
}

const GOOD: &str = r#"{"merchant":"Shop","date":"2024-02-12","currency":"EUR","items":[{"description":"A","quantity":"1","unit_price":null,"total":"1.00"},{"description":"B","quantity":"2","unit_price":"1.25","total":"2.50"}],"total":"3.50"}"#;
const WRONG_SUM: &str = r#"{"merchant":"Shop","date":null,"currency":"EUR","items":[{"description":"A","quantity":"1","unit_price":null,"total":"1.00"}],"total":"3.50"}"#;

#[tokio::test]
async fn a_good_answer_takes_one_request() {
    let (scanner, fake) = serve(vec![(200, GOOD)]).await;
    let outcome = scanner
        .scan(b"jpeg bytes", "image/jpeg", eur())
        .await
        .expect("scan");
    let receipt = outcome.receipt.expect("receipt");
    assert_eq!(check(&receipt, eur()), []);
    assert_eq!(outcome.raw.len(), 1);
    assert_eq!(outcome.usage.input_tokens, 1000);

    let requests = fake.requests.lock().expect("lock");
    let (auth, body) = &requests[0];
    assert_eq!(auth.as_deref(), Some("Bearer test-key"));
    assert_eq!(body["model"], "vision-model");
    assert_eq!(body["response_format"]["type"], "json_schema");
    let url = body["messages"][1]["content"][1]["image_url"]["url"]
        .as_str()
        .expect("url");
    assert_eq!(url, "data:image/jpeg;base64,anBlZyBieXRlcw==");
}

#[tokio::test]
async fn a_wrong_sum_is_repaired_once() {
    let (scanner, fake) = serve(vec![(200, WRONG_SUM), (200, GOOD)]).await;
    let outcome = scanner.scan(b"x", "image/png", eur()).await.expect("scan");
    assert_eq!(outcome.raw.len(), 2);
    assert_eq!(check(outcome.receipt.as_ref().expect("receipt"), eur()), []);
    assert_eq!(outcome.usage.input_tokens, 2000);
    let requests = fake.requests.lock().expect("lock");
    let repair = requests[1].1["messages"][3]["content"]
        .as_str()
        .expect("text");
    assert!(
        repair.contains("add up to 1.00 but the total is 3.50"),
        "{repair}"
    );
}

#[tokio::test]
async fn a_failed_repair_keeps_the_better_answer() {
    let (scanner, _) = serve(vec![(200, WRONG_SUM), (200, "sorry, I can't read it")]).await;
    let outcome = scanner.scan(b"x", "image/png", eur()).await.expect("scan");
    assert_eq!(
        outcome.receipt.expect("receipt").total.as_deref(),
        Some("3.50")
    );

    let (scanner, _) = serve(vec![(200, "nope"), (200, "still nope")]).await;
    assert!(
        scanner
            .scan(b"x", "image/png", eur())
            .await
            .expect("scan")
            .receipt
            .is_none()
    );
}

#[tokio::test]
async fn provider_errors_surface() {
    let (scanner, _) = serve(vec![(401, "bad key")]).await;
    match scanner.scan(b"x", "image/png", eur()).await {
        Err(ScanError::Provider { status: 401, body }) => assert!(body.contains("bad key")),
        other => panic!("{other:?}"),
    }
}
