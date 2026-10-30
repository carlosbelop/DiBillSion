//! Integration tests against a real PostgreSQL. Each test gets its own
//! database. Without `DATABASE_URL` they are skipped, unless
//! `DIBILL_REQUIRE_DB` is set (CI), where a missing database is a failure.

use axum::Router;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use dibill_proto::{EntityKind, MAX_PUSH_OPS, WireOp};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tokio_postgres::{Client, NoTls};
use tower::ServiceExt;
use uuid::Uuid;

use crate::{Pool, app, migrate, pool};

struct TestDb {
    app: Router,
    pool: Pool,
    admin: Client,
    name: String,
}

impl TestDb {
    async fn new() -> Option<Self> {
        let Ok(url) = std::env::var("DATABASE_URL") else {
            assert!(
                std::env::var_os("DIBILL_REQUIRE_DB").is_none(),
                "DIBILL_REQUIRE_DB is set but DATABASE_URL is not"
            );
            eprintln!("skipping: set DATABASE_URL to run the server tests");
            return None;
        };
        let (admin, connection) = tokio_postgres::connect(&url, NoTls).await.unwrap();
        tokio::spawn(connection);
        let name = format!("dibill_test_{}", Uuid::now_v7().simple());
        admin
            .batch_execute(&format!("CREATE DATABASE {name}"))
            .await
            .unwrap();
        let mut config: tokio_postgres::Config = url.parse().unwrap();
        config.dbname(&name);
        let pool = pool(config, 4).unwrap();
        migrate(&pool).await.unwrap();
        // Migrating twice is a no-op.
        migrate(&pool).await.unwrap();
        Some(Self {
            app: app(pool.clone()),
            pool,
            admin,
            name,
        })
    }

    async fn finish(self) {
        self.pool.close();
        drop(self.app);
        self.admin
            .batch_execute(&format!("DROP DATABASE {} WITH (FORCE)", self.name))
            .await
            .unwrap();
    }

    async fn call(
        &self,
        method: Method,
        path: &str,
        token: Option<&str>,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        let mut req = Request::builder().method(method).uri(path);
        if let Some(t) = token {
            req = req.header("authorization", format!("Bearer {t}"));
        }
        let req = match body {
            Some(b) => req
                .header("content-type", "application/json")
                .body(Body::from(b.to_string())),
            None => req.body(Body::empty()),
        }
        .unwrap();
        let res = self.app.clone().oneshot(req).await.unwrap();
        let status = res.status();
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        let value = serde_json::from_slice(&bytes)
            .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into()));
        (status, value)
    }

    /// Registers a new device and returns its id and token.
    async fn device(&self) -> (Uuid, String) {
        let id = Uuid::now_v7();
        let (status, body) = self
            .call(
                Method::POST,
                "/v1/devices",
                None,
                Some(json!({ "device_id": id })),
            )
            .await;
        assert_eq!(status, StatusCode::CREATED);
        (id, body["token"].as_str().unwrap().to_owned())
    }
}

fn op(group: Uuid, device: Uuid, lamport: u64, name: &str) -> WireOp {
    WireOp {
        id: Uuid::now_v7(),
        group,
        lamport,
        device,
        entity: EntityKind::Group,
        entity_id: group,
        field: Some("name".into()),
        value: Some(json!(name).to_string()),
    }
}

fn push_body(ops: &[WireOp]) -> Value {
    json!({ "ops": ops })
}

#[tokio::test]
async fn health() {
    let Some(db) = TestDb::new().await else {
        return;
    };
    let (status, body) = db.call(Method::GET, "/health", None, None).await;
    assert_eq!((status, body), (StatusCode::OK, Value::String("ok".into())));
    db.finish().await;
}

#[tokio::test]
async fn devices_register_once_and_need_a_token() {
    let Some(db) = TestDb::new().await else {
        return;
    };
    let (id, token) = db.device().await;
    assert!(token.len() >= 43, "256-bit token");

    let (status, body) = db
        .call(
            Method::POST,
            "/v1/devices",
            None,
            Some(json!({ "device_id": id })),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["error"], "forbidden");

    let group = Uuid::now_v7();
    let path = format!("/v1/groups/{group}");
    let (status, body) = db.call(Method::PUT, &path, None, None).await;
    assert_eq!(
        (status, &body["error"]),
        (StatusCode::UNAUTHORIZED, &json!("unauthorized"))
    );
    let (status, _) = db.call(Method::PUT, &path, Some("not-a-token"), None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = db.call(Method::PUT, &path, Some(&token), None).await;
    assert_eq!(status, StatusCode::CREATED);
    db.finish().await;
}

#[tokio::test]
async fn only_members_reach_a_group() {
    let Some(db) = TestDb::new().await else {
        return;
    };
    let (a, token_a) = db.device().await;
    let (_, token_b) = db.device().await;
    let group = Uuid::now_v7();
    let path = format!("/v1/groups/{group}");
    let ops_path = format!("{path}/ops");

    let (status, body) = db.call(Method::GET, &ops_path, Some(&token_a), None).await;
    assert_eq!(
        (status, &body["error"]),
        (StatusCode::NOT_FOUND, &json!("not_found"))
    );

    assert_eq!(
        db.call(Method::PUT, &path, Some(&token_a), None).await.0,
        StatusCode::CREATED
    );
    assert_eq!(
        db.call(Method::PUT, &path, Some(&token_a), None).await.0,
        StatusCode::OK
    );
    assert_eq!(
        db.call(Method::PUT, &path, Some(&token_b), None).await.0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        db.call(Method::GET, &ops_path, Some(&token_b), None)
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    let body = push_body(&[op(group, a, 1, "Trip")]);
    assert_eq!(
        db.call(Method::POST, &ops_path, Some(&token_b), Some(body))
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    db.finish().await;
}

#[tokio::test]
async fn push_is_idempotent_and_pull_pages_in_order() {
    let Some(db) = TestDb::new().await else {
        return;
    };
    let (a, token) = db.device().await;
    let group = Uuid::now_v7();
    let ops_path = format!("/v1/groups/{group}/ops");
    db.call(
        Method::PUT,
        &format!("/v1/groups/{group}"),
        Some(&token),
        None,
    )
    .await;

    let mut ops: Vec<WireOp> = (1..=3).map(|n| op(group, a, n, &format!("v{n}"))).collect();
    // A delete travels without field and value.
    ops[2].field = None;
    ops[2].value = None;
    let (status, body) = db
        .call(Method::POST, &ops_path, Some(&token), Some(push_body(&ops)))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, json!({ "accepted": 3, "cursor": 3 }));
    // Retrying the same push changes nothing.
    let (_, body) = db
        .call(Method::POST, &ops_path, Some(&token), Some(push_body(&ops)))
        .await;
    assert_eq!(body, json!({ "accepted": 0, "cursor": 3 }));

    let (status, page) = db
        .call(
            Method::GET,
            &format!("{ops_path}?after=0&limit=2"),
            Some(&token),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(page["more"], true);
    assert_eq!(page["cursor"], 2);
    let first: Vec<WireOp> = page["ops"]
        .as_array()
        .unwrap()
        .iter()
        .map(|o| serde_json::from_value(o["op"].clone()).unwrap())
        .collect();
    assert_eq!(first, ops[..2]);
    assert_eq!(page["ops"][1]["seq"], 2);

    let (_, page) = db
        .call(
            Method::GET,
            &format!("{ops_path}?after=2"),
            Some(&token),
            None,
        )
        .await;
    assert_eq!(page["more"], false);
    assert_eq!(page["cursor"], 3);
    let last: WireOp = serde_json::from_value(page["ops"][0]["op"].clone()).unwrap();
    assert_eq!(last, ops[2]);

    let (_, page) = db
        .call(
            Method::GET,
            &format!("{ops_path}?after=3"),
            Some(&token),
            None,
        )
        .await;
    assert_eq!(page, json!({ "ops": [], "cursor": 3, "more": false }));
    db.finish().await;
}

#[tokio::test]
async fn bad_pushes_are_refused_whole() {
    let Some(db) = TestDb::new().await else {
        return;
    };
    let (a, token) = db.device().await;
    let (b, _) = db.device().await;
    let group = Uuid::now_v7();
    let ops_path = format!("/v1/groups/{group}/ops");
    db.call(
        Method::PUT,
        &format!("/v1/groups/{group}"),
        Some(&token),
        None,
    )
    .await;

    let good = op(group, a, 1, "ok");
    let mut not_mine = op(group, b, 2, "forged");
    let mut other_group = op(Uuid::now_v7(), a, 3, "elsewhere");
    other_group.entity_id = other_group.group;
    let mut half = op(group, a, 4, "x");
    half.value = None;
    for bad in [&mut not_mine, &mut other_group, &mut half] {
        let (status, body) = db
            .call(
                Method::POST,
                &ops_path,
                Some(&token),
                Some(push_body(&[good.clone(), bad.clone()])),
            )
            .await;
        assert_eq!(
            (status, &body["error"]),
            (StatusCode::BAD_REQUEST, &json!("invalid_request")),
            "{bad:?}"
        );
    }
    // Nothing from the refused batches was stored.
    let (_, page) = db.call(Method::GET, &ops_path, Some(&token), None).await;
    assert_eq!(page["ops"], json!([]));

    let too_many: Vec<WireOp> = (0..=MAX_PUSH_OPS as u64)
        .map(|n| op(group, a, n + 1, "n"))
        .collect();
    let (status, body) = db
        .call(
            Method::POST,
            &ops_path,
            Some(&token),
            Some(push_body(&too_many)),
        )
        .await;
    assert_eq!(
        (status, &body["error"]),
        (StatusCode::PAYLOAD_TOO_LARGE, &json!("too_large"))
    );

    let (status, body) = db
        .call(
            Method::POST,
            &ops_path,
            Some(&token),
            Some(json!({ "nope": 1 })),
        )
        .await;
    assert_eq!(
        (status, &body["error"]),
        (StatusCode::BAD_REQUEST, &json!("invalid_request"))
    );
    let (status, _) = db
        .call(Method::GET, "/v1/groups/not-a-uuid/ops", Some(&token), None)
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    db.finish().await;
}

#[tokio::test]
async fn invites_let_others_join_until_turned_off() {
    let Some(db) = TestDb::new().await else {
        return;
    };
    let (a, token_a) = db.device().await;
    let (_, token_b) = db.device().await;
    let (_, token_c) = db.device().await;
    let group = Uuid::now_v7();
    let invites = format!("/v1/groups/{group}/invites");
    let ops_path = format!("/v1/groups/{group}/ops");
    db.call(
        Method::PUT,
        &format!("/v1/groups/{group}"),
        Some(&token_a),
        None,
    )
    .await;
    db.call(
        Method::POST,
        &ops_path,
        Some(&token_a),
        Some(push_body(&[op(group, a, 1, "Trip")])),
    )
    .await;

    // Only the owner gets the code, and it stays the same.
    let (status, body) = db.call(Method::POST, &invites, Some(&token_b), None).await;
    assert_eq!(
        (status, &body["error"]),
        (StatusCode::FORBIDDEN, &json!("forbidden"))
    );
    let (status, body) = db.call(Method::POST, &invites, Some(&token_a), None).await;
    assert_eq!(status, StatusCode::OK);
    let code = body["code"].as_str().unwrap().to_owned();
    assert!(code.len() >= 22, "128-bit code");
    let (_, again) = db.call(Method::POST, &invites, Some(&token_a), None).await;
    assert_eq!(again["code"], code);

    // B joins (twice is fine) and can now pull and push.
    let join = format!("/v1/invites/{code}/join");
    for _ in 0..2 {
        let (status, body) = db.call(Method::POST, &join, Some(&token_b), None).await;
        assert_eq!(
            (status, body),
            (StatusCode::OK, json!({ "group_id": group }))
        );
    }
    let (status, page) = db.call(Method::GET, &ops_path, Some(&token_b), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(page["ops"].as_array().unwrap().len(), 1);
    // Members other than the owner can't manage the link.
    assert_eq!(
        db.call(Method::DELETE, &invites, Some(&token_b), None)
            .await
            .0,
        StatusCode::FORBIDDEN
    );

    // Turned off: the old code stops working; a new one is different.
    assert_eq!(
        db.call(Method::DELETE, &invites, Some(&token_a), None)
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    let (status, body) = db.call(Method::POST, &join, Some(&token_c), None).await;
    assert_eq!(
        (status, &body["error"]),
        (StatusCode::NOT_FOUND, &json!("not_found"))
    );
    let (_, fresh) = db.call(Method::POST, &invites, Some(&token_a), None).await;
    assert_ne!(fresh["code"], code);
    // Joining needs a device token.
    assert_eq!(
        db.call(Method::POST, &join, None, None).await.0,
        StatusCode::UNAUTHORIZED
    );
    db.finish().await;
}

mod end_to_end {
    use dibill_domain::{Currency, ItemShare, MemberId};
    use dibill_store::{
        GroupId, GroupKind, Item, Store, SyncOp, Transaction, TransactionId, TransactionKind,
    };

    use super::*;

    async fn register(db: &TestDb, store: &Store) -> String {
        let id = store.device_id().unwrap().0;
        let (status, body) = db
            .call(
                Method::POST,
                "/v1/devices",
                None,
                Some(json!({ "device_id": id })),
            )
            .await;
        assert_eq!(status, StatusCode::CREATED);
        body["token"].as_str().unwrap().to_owned()
    }

    /// What a sync client will do: send pending ops, then mark them pushed.
    async fn push(db: &TestDb, store: &mut Store, token: &str, group: GroupId) {
        let pending = store.pending_ops(1000).unwrap();
        let wire: Vec<WireOp> = pending.iter().map(WireOp::from).collect();
        let (status, _) = db
            .call(
                Method::POST,
                &format!("/v1/groups/{}/ops", group.0),
                Some(token),
                Some(push_body(&wire)),
            )
            .await;
        assert_eq!(status, StatusCode::OK);
        let ids: Vec<_> = pending.iter().map(|op| op.id).collect();
        store.mark_pushed(&ids).unwrap();
    }

    /// Pulls everything after `cursor`, page by page, and applies it.
    async fn pull(db: &TestDb, store: &mut Store, token: &str, group: GroupId, cursor: &mut u64) {
        loop {
            let (status, page) = db
                .call(
                    Method::GET,
                    &format!("/v1/groups/{}/ops?after={cursor}&limit=3", group.0),
                    Some(token),
                    None,
                )
                .await;
            assert_eq!(status, StatusCode::OK);
            let ops: Vec<SyncOp> = page["ops"]
                .as_array()
                .unwrap()
                .iter()
                .map(|o| {
                    SyncOp::try_from(serde_json::from_value::<WireOp>(o["op"].clone()).unwrap())
                        .unwrap()
                })
                .collect();
            store.apply_remote(&ops).unwrap();
            *cursor = page["cursor"].as_u64().unwrap();
            if page["more"] == false {
                break;
            }
        }
    }

    fn expense(group: GroupId, payer: MemberId, total: i64, among: &[MemberId]) -> Transaction {
        Transaction {
            id: TransactionId::new(),
            group,
            kind: TransactionKind::Expense,
            title: "Dinner".into(),
            payer,
            currency: Currency::from_code("EUR").unwrap(),
            fx_rate: None,
            date_days: 20_000,
            items: vec![Item {
                label: None,
                total,
                shares: among.iter().map(|m| ItemShare::parts(*m, 1)).collect(),
            }],
        }
    }

    fn view(store: &Store, group: GroupId) -> impl PartialEq + std::fmt::Debug {
        (
            store.group(group).unwrap(),
            store.members(group).unwrap(),
            store.transactions(group).unwrap(),
            store.group_balances(group).unwrap(),
        )
    }

    #[tokio::test]
    async fn two_devices_share_a_group_and_converge() {
        let Some(db) = TestDb::new().await else {
            return;
        };
        let mut a = Store::open_in_memory().unwrap();
        let mut b = Store::open_in_memory().unwrap();
        let (token_a, token_b) = (register(&db, &a).await, register(&db, &b).await);

        // A creates and shares a group, then invites B.
        let group = a
            .create_group(
                "Lisbon",
                Some("🏖"),
                GroupKind::Trip,
                Currency::from_code("EUR").unwrap(),
            )
            .unwrap();
        let ana = a.add_member(group, "Ana").unwrap();
        let bea = a.add_member(group, "Bea").unwrap();
        a.save_transaction(&expense(group, ana, 3001, &[ana, bea]))
            .unwrap();
        db.call(
            Method::PUT,
            &format!("/v1/groups/{}", group.0),
            Some(&token_a),
            None,
        )
        .await;
        push(&db, &mut a, &token_a, group).await;
        let (_, invite) = db
            .call(
                Method::POST,
                &format!("/v1/groups/{}/invites", group.0),
                Some(&token_a),
                None,
            )
            .await;
        let (_, joined) = db
            .call(
                Method::POST,
                &format!("/v1/invites/{}/join", invite["code"].as_str().unwrap()),
                Some(&token_b),
                None,
            )
            .await;
        assert_eq!(joined["group_id"], json!(group.0));

        let (mut cursor_a, mut cursor_b) = (0, 0);
        pull(&db, &mut b, &token_b, group, &mut cursor_b).await;
        assert_eq!(view(&b, group), view(&a, group));

        // Both edit offline, then sync in either order.
        b.save_transaction(&expense(group, bea, 1500, &[ana, bea]))
            .unwrap();
        let last = a.transactions(group).unwrap()[0].clone();
        a.delete_transaction(last.id).unwrap();
        a.add_member(group, "Carlos").unwrap();
        push(&db, &mut b, &token_b, group).await;
        push(&db, &mut a, &token_a, group).await;
        pull(&db, &mut a, &token_a, group, &mut cursor_a).await;
        pull(&db, &mut b, &token_b, group, &mut cursor_b).await;

        assert_eq!(view(&a, group), view(&b, group));
        assert_eq!(a.members(group).unwrap().len(), 3);
        let t = a.transactions(group).unwrap();
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].payer, bea);
        assert_eq!(
            a.group_balances(group)
                .unwrap()
                .balances
                .values()
                .sum::<i64>(),
            0
        );
        // Nothing is left to push once both are in sync.
        assert_eq!(a.pending_ops(10).unwrap(), []);
        assert_eq!(b.pending_ops(10).unwrap(), []);
        db.finish().await;
    }
}
