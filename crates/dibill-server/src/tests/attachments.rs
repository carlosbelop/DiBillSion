//! Receipt photos: stored for the group, served to its members only.

use axum::http::{Method, StatusCode};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde_json::json;
use uuid::Uuid;

use super::TestDb;

#[tokio::test]
async fn members_store_and_fetch_photos() {
    let Some(db) = TestDb::new().await else {
        return;
    };
    let (_, owner) = db.device().await;
    let (_, stranger) = db.device().await;
    let group = Uuid::now_v7();
    let (status, _) = db
        .call(
            Method::PUT,
            &format!("/v1/groups/{group}"),
            Some(&owner),
            None,
        )
        .await;
    assert!(status.is_success());
    let path = format!("/v1/groups/{group}/attachments/{}", Uuid::now_v7());
    let photo = json!({ "mime": "image/jpeg", "data_base64": STANDARD.encode("jpeg bytes") });

    let (status, _) = db.call(Method::GET, &path, Some(&owner), None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = db
        .call(Method::PUT, &path, Some(&owner), Some(photo.clone()))
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    // Idempotent: the same upload again is fine and keeps the first copy.
    let again = json!({ "mime": "image/jpeg", "data_base64": STANDARD.encode("other") });
    assert_eq!(
        db.call(Method::PUT, &path, Some(&owner), Some(again))
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    let (status, body) = db.call(Method::GET, &path, Some(&owner), None).await;
    assert_eq!((status, body), (StatusCode::OK, photo.clone()));

    // Not for people outside the group.
    assert_eq!(
        db.call(Method::GET, &path, Some(&stranger), None).await.0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        db.call(Method::PUT, &path, Some(&stranger), Some(photo))
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        db.call(Method::GET, &path, None, None).await.0,
        StatusCode::UNAUTHORIZED
    );

    for bad in [
        json!({ "mime": "image/gif", "data_base64": "AA==" }),
        json!({ "mime": "image/jpeg", "data_base64": "%%%" }),
        json!({ "mime": "image/jpeg", "data_base64": "" }),
    ] {
        let other = format!("/v1/groups/{group}/attachments/{}", Uuid::now_v7());
        assert_eq!(
            db.call(Method::PUT, &other, Some(&owner), Some(bad))
                .await
                .0,
            StatusCode::BAD_REQUEST
        );
    }
    db.finish().await;
}
