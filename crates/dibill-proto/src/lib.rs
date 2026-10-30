//! Wire types of the sync API (ADR 0006), shared by the app core and the
//! server. JSON over HTTPS under `/v1`. The server only checks op envelopes
//! ([`WireOp::check`]); values stay opaque JSON strings decoded by the store.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Path prefix of this protocol version.
pub const API_PREFIX: &str = "/v1";
/// Most ops accepted by one push.
pub const MAX_PUSH_OPS: usize = 1000;
/// Most ops returned by one pull.
pub const MAX_PULL_OPS: u32 = 1000;
/// Largest op value, in bytes.
pub const MAX_VALUE_BYTES: usize = 64 * 1024;
/// Largest request body, in bytes.
pub const MAX_BODY_BYTES: usize = 1024 * 1024;

/// What kind of entity an op changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityKind {
    Group,
    Member,
    Transaction,
}

/// One op as sent over the wire. `field` and `value` are both absent for a delete.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireOp {
    pub id: Uuid,
    pub group: Uuid,
    pub lamport: u64,
    pub device: Uuid,
    pub entity: EntityKind,
    pub entity_id: Uuid,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
}

/// Why an op envelope was rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpProblem {
    /// The counter doesn't fit a signed 64-bit integer.
    Lamport,
    /// A field without a value, or a value without a field.
    FieldWithoutValue,
    /// A field name that is empty or longer than 64 bytes.
    FieldName,
    /// A value over [`MAX_VALUE_BYTES`].
    ValueTooLarge,
    /// A group op whose entity id isn't the group's id.
    GroupEntity,
}

impl WireOp {
    /// Checks the envelope only; values are the client's business.
    ///
    /// # Errors
    /// The first problem found.
    pub fn check(&self) -> Result<(), OpProblem> {
        if i64::try_from(self.lamport).is_err() {
            return Err(OpProblem::Lamport);
        }
        match (&self.field, &self.value) {
            (Some(field), Some(value)) => {
                if field.is_empty() || field.len() > 64 {
                    return Err(OpProblem::FieldName);
                }
                if value.len() > MAX_VALUE_BYTES {
                    return Err(OpProblem::ValueTooLarge);
                }
            }
            (None, None) => {}
            _ => return Err(OpProblem::FieldWithoutValue),
        }
        if self.entity == EntityKind::Group && self.entity_id != self.group {
            return Err(OpProblem::GroupEntity);
        }
        Ok(())
    }
}

/// `POST /v1/devices`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegisterDevice {
    pub device_id: Uuid,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceToken {
    /// Bearer token for every other call. Shown once; the server keeps a hash.
    pub token: String,
}

/// `POST /v1/groups/{id}/ops`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PushOps {
    pub ops: Vec<WireOp>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PushResult {
    /// Ops that were new to the server.
    pub accepted: u32,
    /// The group's latest sequence number after this push.
    pub cursor: u64,
}

/// Query of `GET /v1/groups/{id}/ops`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PullQuery {
    #[serde(default)]
    pub after: u64,
    #[serde(default = "default_limit")]
    pub limit: u32,
}

fn default_limit() -> u32 {
    MAX_PULL_OPS
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeqOp {
    pub seq: u64,
    pub op: WireOp,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PullResult {
    pub ops: Vec<SeqOp>,
    /// Pass as `after` next time.
    pub cursor: u64,
    /// More ops are waiting: pull again.
    pub more: bool,
}

/// `POST /v1/groups/{id}/invites`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Invite {
    pub code: String,
}

/// `POST /v1/invites/{code}/join`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Joined {
    pub group_id: Uuid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    Unauthorized,
    Forbidden,
    NotFound,
    InvalidRequest,
    TooLarge,
}

/// Body of every error response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiError {
    pub error: ErrorCode,
    /// English detail for logs; apps localize by `error`.
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn op() -> WireOp {
        let group = Uuid::from_u128(1);
        WireOp {
            id: Uuid::from_u128(2),
            group,
            lamport: 7,
            device: Uuid::from_u128(3),
            entity: EntityKind::Group,
            entity_id: group,
            field: Some("name".into()),
            value: Some("\"Lisbon\"".into()),
        }
    }

    #[test]
    fn json_shape_is_stable() {
        let json = serde_json::to_string(&op()).unwrap();
        assert_eq!(
            json,
            r#"{"id":"00000000-0000-0000-0000-000000000002","group":"00000000-0000-0000-0000-000000000001","lamport":7,"device":"00000000-0000-0000-0000-000000000003","entity":"group","entity_id":"00000000-0000-0000-0000-000000000001","field":"name","value":"\"Lisbon\""}"#
        );
        assert_eq!(serde_json::from_str::<WireOp>(&json).unwrap(), op());

        let delete = WireOp {
            entity: EntityKind::Transaction,
            entity_id: Uuid::from_u128(9),
            field: None,
            value: None,
            ..op()
        };
        let json = serde_json::to_string(&delete).unwrap();
        assert!(!json.contains("field") && !json.contains("value"));
        assert_eq!(serde_json::from_str::<WireOp>(&json).unwrap(), delete);
    }

    #[test]
    fn envelope_checks() {
        assert_eq!(op().check(), Ok(()));
        let bad = |f: fn(&mut WireOp)| {
            let mut o = op();
            f(&mut o);
            o.check()
        };
        assert_eq!(bad(|o| o.lamport = u64::MAX), Err(OpProblem::Lamport));
        assert_eq!(bad(|o| o.value = None), Err(OpProblem::FieldWithoutValue));
        assert_eq!(bad(|o| o.field = None), Err(OpProblem::FieldWithoutValue));
        assert_eq!(
            bad(|o| o.field = Some(String::new())),
            Err(OpProblem::FieldName)
        );
        assert_eq!(
            bad(|o| o.value = Some("x".repeat(MAX_VALUE_BYTES + 1))),
            Err(OpProblem::ValueTooLarge)
        );
        assert_eq!(
            bad(|o| o.entity_id = Uuid::from_u128(5)),
            Err(OpProblem::GroupEntity)
        );
    }

    #[test]
    fn pull_query_defaults() {
        let q: PullQuery = serde_json::from_str("{}").unwrap();
        assert_eq!(
            q,
            PullQuery {
                after: 0,
                limit: MAX_PULL_OPS
            }
        );
        let e = ApiError {
            error: ErrorCode::NotFound,
            message: "no such group".into(),
        };
        assert_eq!(
            serde_json::to_string(&e).unwrap(),
            r#"{"error":"not_found","message":"no such group"}"#
        );
    }
}
