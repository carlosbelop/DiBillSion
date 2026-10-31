//! Sync client (ADR 0007). The protocol (ADR 0006) runs here; the platform only
//! performs raw HTTP requests through [`HttpTransport`], with its own TLS and
//! trust store. The store lock is never held while a request is in flight.

use std::sync::Arc;

use dibill_proto::{
    ApiError, DeviceToken, Invite, Joined, MAX_PUSH_OPS, PullResult, PushOps, RegisterDevice,
    WireOp,
};
use dibill_store::{GroupId, SyncOp};
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::{Core, CoreError, uuid};

/// One HTTP request for the platform to perform. `path` starts with `/v1/`
/// or is `/health`; the platform prefixes its server URL.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct HttpRequest {
    pub method: String,
    pub path: String,
    /// Bearer token to send as `Authorization`.
    pub token: Option<String>,
    /// JSON body.
    pub body: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct HttpResponse {
    pub status: u16,
    pub body: String,
}

/// The request never got an HTTP answer (no network, timeout, TLS failure...).
#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum TransportError {
    #[error("offline: {reason}")]
    Offline { reason: String },
}

impl From<uniffi::UnexpectedUniFFICallbackError> for TransportError {
    fn from(e: uniffi::UnexpectedUniFFICallbackError) -> Self {
        Self::Offline { reason: e.reason }
    }
}

/// Implemented by the app (`OkHttp`/`HttpURLConnection` on Android, `URLSession` on iOS).
/// Called from the same background thread that called the core.
#[uniffi::export(with_foreign)]
pub trait HttpTransport: Send + Sync {
    /// # Errors
    /// [`TransportError::Offline`] when there is no HTTP response at all.
    fn request(&self, request: HttpRequest) -> Result<HttpResponse, TransportError>;
}

/// What one sync round did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, uniffi::Record)]
pub struct SyncSummary {
    pub pushed: u32,
    pub pulled: u32,
}

impl From<TransportError> for CoreError {
    fn from(e: TransportError) -> Self {
        match e {
            TransportError::Offline { reason } => Self::Offline { reason },
        }
    }
}

fn parse_group(id: &str) -> Result<GroupId, CoreError> {
    uuid(id).map(GroupId)
}

pub(crate) fn json<T: Serialize>(value: &T) -> Result<String, CoreError> {
    serde_json::to_string(value).map_err(|e| CoreError::InvalidInput {
        reason: e.to_string(),
    })
}

pub(crate) fn parse<T: DeserializeOwned>(body: &str) -> Result<T, CoreError> {
    serde_json::from_str(body).map_err(|e| CoreError::Server {
        code: "invalid_response".into(),
        reason: e.to_string(),
    })
}

#[uniffi::export]
impl Core {
    /// Shares a local group through the server: registers this device if
    /// needed, creates the group there and uploads everything.
    ///
    /// # Errors
    /// [`CoreError::Offline`], [`CoreError::Server`], or storage errors.
    pub fn share_group(
        &self,
        transport: Arc<dyn HttpTransport>,
        group_id: String,
    ) -> Result<SyncSummary, CoreError> {
        let group = group_id_checked(self, &group_id)?;
        let token = self.ensure_registered(&*transport)?;
        send(
            &*transport,
            "PUT",
            format!("/v1/groups/{}", group.0),
            Some(&token),
            None,
        )?;
        self.store().mark_shared(group)?;
        self.sync_group(&*transport, &token, group)
    }

    /// Whether a group is shared through the server.
    ///
    /// # Errors
    /// [`CoreError::InvalidInput`] for a malformed id.
    pub fn is_shared(&self, group_id: String) -> Result<bool, CoreError> {
        Ok(self.store().is_shared(parse_group(&group_id)?)?)
    }

    /// The group's invite code (only its owner can get it).
    ///
    /// # Errors
    /// [`CoreError::Server`] with code `forbidden` for non-owners.
    pub fn invite_code(
        &self,
        transport: Arc<dyn HttpTransport>,
        group_id: String,
    ) -> Result<String, CoreError> {
        let group = parse_group(&group_id)?;
        let token = self.ensure_registered(&*transport)?;
        let body = send(
            &*transport,
            "POST",
            format!("/v1/groups/{}/invites", group.0),
            Some(&token),
            None,
        )?;
        Ok(parse::<Invite>(&body)?.code)
    }

    /// Turns the invite link off; asking for a code again makes a new one.
    ///
    /// # Errors
    /// [`CoreError::Server`] with code `forbidden` for non-owners.
    pub fn turn_off_invite(
        &self,
        transport: Arc<dyn HttpTransport>,
        group_id: String,
    ) -> Result<(), CoreError> {
        let group = parse_group(&group_id)?;
        let token = self.ensure_registered(&*transport)?;
        send(
            &*transport,
            "DELETE",
            format!("/v1/groups/{}/invites", group.0),
            Some(&token),
            None,
        )?;
        Ok(())
    }

    /// Joins a group with an invite code and downloads it. Returns its id.
    ///
    /// # Errors
    /// [`CoreError::Server`] with code `not_found` for an invalid code.
    pub fn join_group(
        &self,
        transport: Arc<dyn HttpTransport>,
        code: String,
    ) -> Result<String, CoreError> {
        let code = code.trim();
        if code.is_empty()
            || !code
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return Err(CoreError::InvalidInput {
                reason: "an invite code is letters, digits, '-' and '_'".into(),
            });
        }
        let token = self.ensure_registered(&*transport)?;
        let body = send(
            &*transport,
            "POST",
            format!("/v1/invites/{code}/join"),
            Some(&token),
            None,
        )?;
        let group = GroupId(parse::<Joined>(&body)?.group_id);
        self.store().mark_shared(group)?;
        self.sync_group(&*transport, &token, group)?;
        Ok(group.0.to_string())
    }

    /// Pushes local changes and pulls remote ones for every shared group.
    ///
    /// # Errors
    /// The first failure; groups synced before it keep their progress.
    pub fn sync(&self, transport: Arc<dyn HttpTransport>) -> Result<SyncSummary, CoreError> {
        let groups = self.store().shared_groups()?;
        if groups.is_empty() {
            return Ok(SyncSummary::default());
        }
        let token = self.ensure_registered(&*transport)?;
        let mut total = SyncSummary::default();
        for (group, _) in groups {
            let s = self.sync_group(&*transport, &token, group)?;
            total.pushed += s.pushed;
            total.pulled += s.pulled;
        }
        Ok(total)
    }
}

fn group_id_checked(core: &Core, id: &str) -> Result<GroupId, CoreError> {
    let group = parse_group(id)?;
    core.store().group(group)?;
    Ok(group)
}

/// Sends one request; non-2xx answers become [`CoreError::Server`].
pub(crate) fn send(
    transport: &dyn HttpTransport,
    method: &str,
    path: String,
    token: Option<&str>,
    body: Option<String>,
) -> Result<String, CoreError> {
    let res = transport.request(HttpRequest {
        method: method.to_owned(),
        path,
        token: token.map(str::to_owned),
        body,
    })?;
    if (200..300).contains(&res.status) {
        return Ok(res.body);
    }
    Err(match serde_json::from_str::<ApiError>(&res.body) {
        Ok(e) => CoreError::Server {
            code: serde_json::to_value(e.error)
                .ok()
                .and_then(|v| v.as_str().map(str::to_owned))
                .unwrap_or_default(),
            reason: e.message,
        },
        Err(_) => CoreError::Server {
            code: format!("http_{}", res.status),
            reason: res.body,
        },
    })
}

impl Core {
    /// This device's token, registering it on first use.
    pub(crate) fn ensure_registered(
        &self,
        transport: &dyn HttpTransport,
    ) -> Result<String, CoreError> {
        let (token, device) = {
            let store = self.store();
            (store.sync_token()?, store.device_id()?)
        };
        if let Some(token) = token {
            return Ok(token);
        }
        let body = send(
            transport,
            "POST",
            "/v1/devices".into(),
            None,
            Some(json(&RegisterDevice {
                device_id: device.0,
            })?),
        )?;
        let token = parse::<DeviceToken>(&body)?.token;
        self.store().set_sync_token(&token)?;
        Ok(token)
    }

    /// Pushes this device's pending ops of `group`, then pulls everything new.
    fn sync_group(
        &self,
        transport: &dyn HttpTransport,
        token: &str,
        group: GroupId,
    ) -> Result<SyncSummary, CoreError> {
        let path = format!("/v1/groups/{}/ops", group.0);
        let mut summary = SyncSummary::default();
        let batch = u32::try_from(MAX_PUSH_OPS).unwrap_or(u32::MAX);
        loop {
            let pending = self.store().pending_group_ops(group, batch)?;
            if pending.is_empty() {
                break;
            }
            let ops: Vec<WireOp> = pending.iter().map(WireOp::from).collect();
            send(
                transport,
                "POST",
                path.clone(),
                Some(token),
                Some(json(&PushOps { ops })?),
            )?;
            let ids: Vec<_> = pending.iter().map(|op| op.id).collect();
            self.store().mark_pushed(&ids)?;
            summary.pushed += u32::try_from(ids.len()).unwrap_or(u32::MAX);
        }

        let mut cursor = self
            .store()
            .shared_groups()?
            .into_iter()
            .find_map(|(g, c)| (g == group).then_some(c))
            .unwrap_or(0);
        loop {
            let body = send(
                transport,
                "GET",
                format!("{path}?after={cursor}"),
                Some(token),
                None,
            )?;
            let page: PullResult = parse(&body)?;
            // Ops this version can't read are skipped like malformed ones (ADR 0005).
            let ops: Vec<SyncOp> = page
                .ops
                .into_iter()
                .filter_map(|o| SyncOp::try_from(o.op).ok())
                .collect();
            {
                let mut store = self.store();
                summary.pulled += u32::try_from(store.apply_remote(&ops)?).unwrap_or(u32::MAX);
                store.set_cursor(group, page.cursor)?;
            }
            cursor = page.cursor;
            if !page.more {
                break;
            }
        }
        Ok(summary)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicBool, Ordering};

    use dibill_proto::{PullResult, SeqOp};
    use uuid::Uuid;

    use super::*;
    use crate::{GroupKindDto, ItemDto, ShareDto, TransactionDto, TransactionKindDto};

    /// An in-memory stand-in for `dibill-server` that follows ADR 0006, with
    /// tiny pages so paging is exercised.
    #[derive(Default)]
    struct FakeServer {
        offline: AtomicBool,
        state: Mutex<FakeState>,
    }

    #[derive(Default)]
    struct FakeState {
        tokens: BTreeMap<String, Uuid>,
        groups: BTreeMap<Uuid, FakeGroup>,
    }

    struct FakeGroup {
        owner: Uuid,
        members: Vec<Uuid>,
        ops: Vec<WireOp>,
        code: Option<String>,
    }

    const PAGE: usize = 2;

    impl FakeGroup {
        /// Calls on a group the caller is a member of.
        fn handle(
            &mut self,
            id: Uuid,
            me: Uuid,
            method: &str,
            rest: &[&str],
            query: &str,
            body: Option<&str>,
        ) -> HttpResponse {
            match (method, rest) {
                ("PUT", []) => ok(200, ()),
                ("POST", ["ops"]) => {
                    let req: PushOps = serde_json::from_str(body.unwrap()).unwrap();
                    if req.ops.iter().any(|o| o.device != me || o.group != id) {
                        return err(400, "invalid_request");
                    }
                    for op in req.ops {
                        if !self.ops.iter().any(|o| o.id == op.id) {
                            self.ops.push(op);
                        }
                    }
                    ok(
                        200,
                        serde_json::json!({ "accepted": 0, "cursor": self.ops.len() }),
                    )
                }
                ("GET", ["ops"]) => {
                    let after: usize = query.trim_start_matches("after=").parse().unwrap();
                    let ops: Vec<SeqOp> = (after..self.ops.len().min(after + PAGE))
                        .map(|i| SeqOp {
                            seq: u64::try_from(i + 1).unwrap(),
                            op: self.ops[i].clone(),
                        })
                        .collect();
                    let next = after + ops.len();
                    ok(
                        200,
                        PullResult {
                            ops,
                            cursor: u64::try_from(next).unwrap(),
                            more: next < self.ops.len(),
                        },
                    )
                }
                ("POST", ["invites"]) if self.owner == me => {
                    let code = self.code.get_or_insert_with(|| format!("code{id}")).clone();
                    ok(200, Invite { code })
                }
                ("DELETE", ["invites"]) if self.owner == me => {
                    self.code = None;
                    ok(204, ())
                }
                _ => err(403, "forbidden"),
            }
        }
    }

    fn ok(status: u16, body: impl Serialize) -> HttpResponse {
        HttpResponse {
            status,
            body: serde_json::to_string(&body).unwrap(),
        }
    }

    fn err(status: u16, code: &str) -> HttpResponse {
        ok(
            status,
            serde_json::json!({ "error": code, "message": code }),
        )
    }

    impl HttpTransport for FakeServer {
        fn request(&self, r: HttpRequest) -> Result<HttpResponse, TransportError> {
            if self.offline.load(Ordering::SeqCst) {
                return Err(TransportError::Offline {
                    reason: "airplane mode".into(),
                });
            }
            let mut s = self.state.lock().unwrap();
            let (path, query) = r.path.split_once('?').unwrap_or((&r.path, ""));
            let parts: Vec<&str> = path.trim_start_matches("/v1/").split('/').collect();
            if r.method == "POST" && parts == ["devices"] {
                let req: RegisterDevice = serde_json::from_str(r.body.as_deref().unwrap()).unwrap();
                if s.tokens.values().any(|d| *d == req.device_id) {
                    return Ok(err(403, "forbidden"));
                }
                let token = format!("t{}", s.tokens.len());
                s.tokens.insert(token.clone(), req.device_id);
                return Ok(ok(201, DeviceToken { token }));
            }
            let Some(me) = r.token.as_ref().and_then(|t| s.tokens.get(t)).copied() else {
                return Ok(err(401, "unauthorized"));
            };
            match (r.method.as_str(), parts.as_slice()) {
                ("POST", ["invites", code, "join"]) => {
                    let Some((id, g)) = s
                        .groups
                        .iter_mut()
                        .find(|(_, g)| g.code.as_deref() == Some(*code))
                    else {
                        return Ok(err(404, "not_found"));
                    };
                    if !g.members.contains(&me) {
                        g.members.push(me);
                    }
                    Ok(ok(200, Joined { group_id: *id }))
                }
                (method, ["groups", id, rest @ ..]) => {
                    let id: Uuid = id.parse().unwrap();
                    if (method, rest) == ("PUT", &[][..]) && !s.groups.contains_key(&id) {
                        s.groups.insert(
                            id,
                            FakeGroup {
                                owner: me,
                                members: vec![me],
                                ops: Vec::new(),
                                code: None,
                            },
                        );
                        return Ok(ok(201, ()));
                    }
                    let Some(g) = s.groups.get_mut(&id) else {
                        return Ok(err(404, "not_found"));
                    };
                    if !g.members.contains(&me) {
                        return Ok(err(403, "forbidden"));
                    }
                    Ok(g.handle(id, me, method, rest, query, r.body.as_deref()))
                }
                _ => Ok(err(404, "not_found")),
            }
        }
    }

    fn expense(group: &str, payer: &str, total: i64, among: &[&String]) -> TransactionDto {
        TransactionDto {
            id: None,
            group_id: group.to_owned(),
            kind: TransactionKindDto::Expense,
            title: "Groceries".into(),
            payer_id: payer.to_owned(),
            currency: "EUR".into(),
            fx_rate: None,
            date_days: 20_000,
            items: vec![ItemDto {
                label: None,
                total_minor: total,
                shares: among
                    .iter()
                    .map(|m| ShareDto {
                        member_id: (*m).clone(),
                        parts: Some(1),
                        amount_minor: None,
                    })
                    .collect(),
            }],
            base_total_minor: None,
        }
    }

    fn same_group(a: &Core, b: &Core, group: &str) {
        let name = |c: &Core| {
            c.groups()
                .unwrap()
                .into_iter()
                .find(|g| g.id == group)
                .map(|g| g.name)
        };
        assert_eq!(name(a), name(b));
        assert_eq!(
            a.members(group.into()).unwrap(),
            b.members(group.into()).unwrap()
        );
        assert_eq!(
            a.transactions(group.into()).unwrap(),
            b.transactions(group.into()).unwrap()
        );
        assert_eq!(
            a.balances(group.into()).unwrap(),
            b.balances(group.into()).unwrap()
        );
    }

    #[test]
    fn share_invite_join_and_converge() {
        let server: Arc<FakeServer> = Arc::default();
        let a = Core::in_memory().unwrap();
        let b = Core::in_memory().unwrap();
        let group = a
            .create_group("Flat".into(), None, GroupKindDto::Home, "EUR".into())
            .unwrap();
        let ana = a.add_member(group.clone(), "Ana".into()).unwrap();
        let bea = a.add_member(group.clone(), "Bea".into()).unwrap();
        a.save_transaction(expense(&group, &ana, 1001, &[&ana, &bea]))
            .unwrap();

        assert!(!a.is_shared(group.clone()).unwrap());
        let shared = a.share_group(server.clone(), group.clone()).unwrap();
        assert_eq!(shared.pushed, 4 + 2 * 2 + 1);
        assert!(a.is_shared(group.clone()).unwrap());
        let code = a.invite_code(server.clone(), group.clone()).unwrap();
        assert_eq!(a.invite_code(server.clone(), group.clone()).unwrap(), code);

        // B can't manage A's link, but can join with it.
        assert_eq!(
            b.join_group(server.clone(), format!("  {code} ")).unwrap(),
            group
        );
        assert!(matches!(
            b.invite_code(server.clone(), group.clone()),
            Err(CoreError::Server { code, .. }) if code == "forbidden"
        ));
        same_group(&a, &b, &group);

        // Both edit, then sync in any order.
        b.save_transaction(expense(&group, &bea, 600, &[&ana, &bea]))
            .unwrap();
        a.add_member(group.clone(), "Carlos".into()).unwrap();
        let first = a.transactions(group.clone()).unwrap()[0]
            .id
            .clone()
            .unwrap();
        a.delete_transaction(first).unwrap();
        assert_eq!(b.sync(server.clone()).unwrap().pushed, 1);
        a.sync(server.clone()).unwrap();
        b.sync(server.clone()).unwrap();
        same_group(&a, &b, &group);
        assert_eq!(a.members(group.clone()).unwrap().len(), 3);
        assert_eq!(a.transactions(group.clone()).unwrap().len(), 1);
        // Nothing left to do.
        assert_eq!(a.sync(server.clone()).unwrap(), SyncSummary::default());
    }

    #[test]
    fn offline_changes_stay_queued() {
        let server: Arc<FakeServer> = Arc::default();
        let a = Core::in_memory().unwrap();
        let b = Core::in_memory().unwrap();
        let group = a
            .create_group("Trip".into(), None, GroupKindDto::Trip, "EUR".into())
            .unwrap();
        a.share_group(server.clone(), group.clone()).unwrap();
        let code = a.invite_code(server.clone(), group.clone()).unwrap();
        b.join_group(server.clone(), code).unwrap();

        server.offline.store(true, Ordering::SeqCst);
        a.add_member(group.clone(), "Dani".into()).unwrap();
        assert!(matches!(
            a.sync(server.clone()),
            Err(CoreError::Offline { .. })
        ));
        server.offline.store(false, Ordering::SeqCst);
        assert_eq!(a.sync(server.clone()).unwrap().pushed, 2);
        b.sync(server.clone()).unwrap();
        same_group(&a, &b, &group);
    }

    #[test]
    fn bad_invites_and_turned_off_links() {
        let server: Arc<FakeServer> = Arc::default();
        let a = Core::in_memory().unwrap();
        let b = Core::in_memory().unwrap();
        assert!(matches!(
            b.join_group(server.clone(), "not a code!".into()),
            Err(CoreError::InvalidInput { .. })
        ));
        assert!(matches!(
            b.join_group(server.clone(), "nope".into()),
            Err(CoreError::Server { code, .. }) if code == "not_found"
        ));
        let group = a
            .create_group("Event".into(), None, GroupKindDto::Event, "EUR".into())
            .unwrap();
        // Sharing an unknown group fails locally, before any request.
        assert!(matches!(
            a.share_group(server.clone(), Uuid::nil().to_string()),
            Err(CoreError::NotFound)
        ));
        a.share_group(server.clone(), group.clone()).unwrap();
        let code = a.invite_code(server.clone(), group.clone()).unwrap();
        a.turn_off_invite(server.clone(), group.clone()).unwrap();
        assert!(matches!(
            b.join_group(server.clone(), code),
            Err(CoreError::Server { code, .. }) if code == "not_found"
        ));
    }
}
