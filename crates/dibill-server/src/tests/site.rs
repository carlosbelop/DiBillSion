//! Invite links: the join page, the app-link files and the link returned with an invite.

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;
use uuid::Uuid;

use super::TestDb;
use crate::{Receipts, Site};

fn site() -> Site {
    Site {
        url: Some("https://example.test".into()),
        ios_apps: vec!["TEAM123.com.dibillsion.app".into()],
        android_apps: vec![("com.dibillsion.app".into(), "AA:BB".into())],
    }
}

async fn get(db: &TestDb, path: &str, language: Option<&str>) -> (StatusCode, String, String) {
    let mut req = Request::builder().method(Method::GET).uri(path);
    if let Some(l) = language {
        req = req.header("accept-language", l);
    }
    let res = db
        .app
        .clone()
        .oneshot(req.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = res.status();
    let kind = res
        .headers()
        .get("content-type")
        .map(|v| v.to_str().unwrap().to_owned())
        .unwrap_or_default();
    let body = res.into_body().collect().await.unwrap().to_bytes();
    (status, kind, String::from_utf8_lossy(&body).into_owned())
}

#[tokio::test]
async fn join_page_in_the_readers_language() {
    let Some(db) = TestDb::with_site(Receipts::disabled(), site()).await else {
        return;
    };
    let (status, kind, page) = get(&db, "/join/abc-DEF_123", None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(kind.starts_with("text/html"));
    assert!(page.contains("dibillsion://join/abc-DEF_123"));
    assert!(page.contains("Open in DiBillSion"));
    let (_, _, page) = get(&db, "/join/abc", Some("es-ES,es;q=0.9,en;q=0.8")).await;
    assert!(page.contains("Abrir en DiBillSion"));
    for bad in ["/join/%3Cscript%3E", "/join/a%20b"] {
        assert_eq!(get(&db, bad, None).await.0, StatusCode::NOT_FOUND, "{bad}");
    }
    db.finish().await;
}

#[tokio::test]
async fn app_link_files() {
    let Some(db) = TestDb::with_site(Receipts::disabled(), site()).await else {
        return;
    };
    let (status, kind, body) = get(&db, "/.well-known/apple-app-site-association", None).await;
    assert_eq!(
        (status, kind.as_str()),
        (StatusCode::OK, "application/json")
    );
    let aasa: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(
        aasa["applinks"]["details"][0]["appIDs"][0],
        "TEAM123.com.dibillsion.app"
    );
    assert_eq!(
        aasa["applinks"]["details"][0]["components"][0]["/"],
        "/join/*"
    );

    let (status, _, body) = get(&db, "/.well-known/assetlinks.json", None).await;
    assert_eq!(status, StatusCode::OK);
    let links: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(links[0]["target"]["package_name"], "com.dibillsion.app");
    assert_eq!(links[0]["target"]["sha256_cert_fingerprints"][0], "AA:BB");
    db.finish().await;
}

#[tokio::test]
async fn invites_carry_the_link() {
    let Some(db) = TestDb::with_site(Receipts::disabled(), site()).await else {
        return;
    };
    let (_, token) = db.device().await;
    let group = Uuid::now_v7();
    let (status, _) = db
        .call(
            Method::PUT,
            &format!("/v1/groups/{group}"),
            Some(&token),
            None,
        )
        .await;
    assert!(status.is_success());
    let (status, invite) = db
        .call(
            Method::POST,
            &format!("/v1/groups/{group}/invites"),
            Some(&token),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let code = invite["code"].as_str().unwrap();
    assert_eq!(invite["link"], format!("https://example.test/join/{code}"));
    db.finish().await;
}
