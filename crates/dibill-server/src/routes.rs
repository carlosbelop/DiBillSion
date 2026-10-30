use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use deadpool_postgres::GenericClient;
use dibill_proto::{
    DeviceToken, EntityKind, MAX_PULL_OPS, MAX_PUSH_OPS, PullQuery, PullResult, PushOps,
    PushResult, RegisterDevice, SeqOp, WireOp,
};
use tokio_postgres::Row;
use uuid::Uuid;

use crate::auth::{Device, hash, new_token};
use crate::extract::{Body, Path, Query};
use crate::{AppError, AppState};

pub async fn health(State(s): State<AppState>) -> Result<&'static str, AppError> {
    s.db.get().await?.simple_query("SELECT 1").await?;
    Ok("ok")
}

/// `POST /v1/devices`: a token for a new device id. An id can be registered once.
pub async fn register_device(
    State(s): State<AppState>,
    Body(req): Body<RegisterDevice>,
) -> Result<(StatusCode, Json<DeviceToken>), AppError> {
    let token = new_token(32)?;
    let inserted =
        s.db.get()
            .await?
            .execute(
                "INSERT INTO devices (id, token_hash) VALUES ($1, $2) ON CONFLICT (id) DO NOTHING",
                &[&req.device_id, &hash(&token)],
            )
            .await?;
    if inserted == 0 {
        return Err(AppError::Forbidden("this device is already registered"));
    }
    Ok((StatusCode::CREATED, Json(DeviceToken { token })))
}

/// `PUT /v1/groups/{id}`: shares a group. Idempotent for its members.
pub async fn share_group(
    State(s): State<AppState>,
    Device(me): Device,
    Path(group): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let mut client = s.db.get().await?;
    let tx = client.transaction().await?;
    let created = tx
        .execute(
            "INSERT INTO groups (id, owner) VALUES ($1, $2) ON CONFLICT (id) DO NOTHING",
            &[&group, &me],
        )
        .await?
        == 1;
    if created {
        tx.execute(
            "INSERT INTO group_members (group_id, device_id) VALUES ($1, $2)",
            &[&group, &me],
        )
        .await?;
        tx.commit().await?;
        return Ok(StatusCode::CREATED);
    }
    require_member(&tx, group, me).await?;
    Ok(StatusCode::OK)
}

/// `Ok` for members; `NotFound` for unknown groups, `Forbidden` for others.
async fn require_member(
    db: &impl GenericClient,
    group: Uuid,
    device: Uuid,
) -> Result<(), AppError> {
    let row = db
        .query_opt(
            "SELECT EXISTS (SELECT 1 FROM group_members WHERE group_id = $1 AND device_id = $2)
             FROM groups WHERE id = $1",
            &[&group, &device],
        )
        .await?;
    match row.map(|r| r.get::<_, bool>(0)) {
        None => Err(AppError::NotFound("no such group")),
        Some(false) => Err(AppError::Forbidden("not a member of this group")),
        Some(true) => Ok(()),
    }
}

fn entity_str(kind: EntityKind) -> &'static str {
    match kind {
        EntityKind::Group => "group",
        EntityKind::Member => "member",
        EntityKind::Transaction => "transaction",
    }
}

fn to_i64(n: u64, what: &str) -> Result<i64, AppError> {
    i64::try_from(n).map_err(|_| AppError::Invalid(format!("{what} out of range")))
}

/// `POST /v1/groups/{id}/ops`: stores new ops in the group's order. A batch
/// with any invalid op is refused whole.
pub async fn push_ops(
    State(s): State<AppState>,
    Device(me): Device,
    Path(group): Path<Uuid>,
    Body(req): Body<PushOps>,
) -> Result<Json<PushResult>, AppError> {
    if req.ops.len() > MAX_PUSH_OPS {
        return Err(AppError::TooLarge(format!(
            "at most {MAX_PUSH_OPS} ops per push"
        )));
    }
    for op in &req.ops {
        if op.group != group {
            return Err(AppError::Invalid(format!(
                "op {} belongs to another group",
                op.id
            )));
        }
        if op.device != me {
            return Err(AppError::Invalid(format!(
                "op {} was not made by this device",
                op.id
            )));
        }
        op.check()
            .map_err(|p| AppError::Invalid(format!("op {}: {p:?}", op.id)))?;
    }

    let mut client = s.db.get().await?;
    let tx = client.transaction().await?;
    require_member(&tx, group, me).await?;
    // Locks the group's row, so concurrent pushes get consecutive numbers.
    let mut seq: i64 = tx
        .query_one(
            "SELECT last_seq FROM groups WHERE id = $1 FOR UPDATE",
            &[&group],
        )
        .await?
        .get(0);
    let insert = tx
        .prepare_cached(
            "INSERT INTO ops (group_id, seq, id, lamport, device, entity, entity_id, field, value)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) ON CONFLICT (id) DO NOTHING",
        )
        .await?;
    let mut accepted = 0u32;
    for op in &req.ops {
        let inserted = tx
            .execute(
                &insert,
                &[
                    &group,
                    &(seq + 1),
                    &op.id,
                    &to_i64(op.lamport, "lamport")?,
                    &op.device,
                    &entity_str(op.entity),
                    &op.entity_id,
                    &op.field.as_deref(),
                    &op.value.as_deref(),
                ],
            )
            .await?;
        if inserted == 1 {
            seq += 1;
            accepted += 1;
        }
    }
    tx.execute(
        "UPDATE groups SET last_seq = $2 WHERE id = $1",
        &[&group, &seq],
    )
    .await?;
    tx.commit().await?;
    Ok(Json(PushResult {
        accepted,
        cursor: seq.unsigned_abs(),
    }))
}

fn seq_op(group: Uuid, row: &Row) -> Result<SeqOp, AppError> {
    let entity = match row.get::<_, &str>("entity") {
        "group" => EntityKind::Group,
        "member" => EntityKind::Member,
        "transaction" => EntityKind::Transaction,
        other => return Err(AppError::Internal(format!("stored entity {other:?}"))),
    };
    Ok(SeqOp {
        seq: row.get::<_, i64>("seq").unsigned_abs(),
        op: WireOp {
            id: row.get("id"),
            group,
            lamport: row.get::<_, i64>("lamport").unsigned_abs(),
            device: row.get("device"),
            entity,
            entity_id: row.get("entity_id"),
            field: row.get("field"),
            value: row.get("value"),
        },
    })
}

/// `GET /v1/groups/{id}/ops?after=&limit=`: ops after a cursor, in order.
pub async fn pull_ops(
    State(s): State<AppState>,
    Device(me): Device,
    Path(group): Path<Uuid>,
    Query(q): Query<PullQuery>,
) -> Result<Json<PullResult>, AppError> {
    let client = s.db.get().await?;
    require_member(&client, group, me).await?;
    let limit = q.limit.clamp(1, MAX_PULL_OPS);
    let rows = client
        .query(
            "SELECT seq, id, lamport, device, entity, entity_id, field, value
             FROM ops WHERE group_id = $1 AND seq > $2 ORDER BY seq LIMIT $3",
            &[&group, &to_i64(q.after, "after")?, &(i64::from(limit) + 1)],
        )
        .await?;
    let more = rows.len() > limit as usize;
    let ops = rows
        .iter()
        .take(limit as usize)
        .map(|r| seq_op(group, r))
        .collect::<Result<Vec<_>, _>>()?;
    let cursor = ops.last().map_or(q.after, |o| o.seq);
    Ok(Json(PullResult { ops, cursor, more }))
}
