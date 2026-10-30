//! The sync relay (ADR 0006): registers devices, lets them share groups and
//! relays ops between member devices in a per-group order. It never reads op
//! values; every money rule runs on the devices.

mod auth;
mod error;
mod extract;
mod routes;

use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::routing::{get, post, put};
use deadpool_postgres::{Manager, ManagerConfig, RecyclingMethod};
use dibill_proto::{API_PREFIX, MAX_BODY_BYTES};
use tokio_postgres::NoTls;
use tower_http::compression::CompressionLayer;
use tower_http::trace::TraceLayer;

pub use deadpool_postgres::Pool;
pub use error::AppError;

/// Schema migrations, applied in order. Never edit a released one: append.
const MIGRATIONS: &[&str] = &[include_str!("../migrations/0001_sync.sql")];

/// Arbitrary key for the advisory lock that serializes concurrent migrations.
const MIGRATION_LOCK: i64 = 0x0D1B_1115;

#[derive(Debug, Clone)]
pub struct AppState {
    pub db: Pool,
}

/// A connection pool for `config` (no TLS: the database is reached on a private network).
///
/// # Errors
/// [`AppError::Internal`] if the pool can't be built.
pub fn pool(config: tokio_postgres::Config, max_size: usize) -> Result<Pool, AppError> {
    let manager = Manager::from_config(
        config,
        NoTls,
        ManagerConfig {
            recycling_method: RecyclingMethod::Fast,
        },
    );
    Pool::builder(manager)
        .max_size(max_size)
        .build()
        .map_err(|e| AppError::Internal(format!("database pool: {e}")))
}

/// Brings the schema up to date, once even if several servers start together.
///
/// # Errors
/// Database errors; nothing is applied then.
pub async fn migrate(pool: &Pool) -> Result<(), AppError> {
    let mut client = pool.get().await?;
    let tx = client.transaction().await?;
    tx.execute("SELECT pg_advisory_xact_lock($1)", &[&MIGRATION_LOCK])
        .await?;
    tx.batch_execute(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
             version    INTEGER PRIMARY KEY,
             applied_at TIMESTAMPTZ NOT NULL DEFAULT now()
         )",
    )
    .await?;
    let done: i32 = tx
        .query_one(
            "SELECT coalesce(max(version), 0) FROM schema_migrations",
            &[],
        )
        .await?
        .get(0);
    for (version, sql) in (1_i32..)
        .zip(MIGRATIONS)
        .skip(usize::try_from(done).unwrap_or(0))
    {
        tx.batch_execute(sql).await?;
        tx.execute(
            "INSERT INTO schema_migrations (version) VALUES ($1)",
            &[&version],
        )
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

/// The whole HTTP API on a migrated database.
pub fn app(db: Pool) -> Router {
    let api = Router::new()
        .route("/devices", post(routes::register_device))
        .route("/groups/{id}", put(routes::share_group))
        .route(
            "/groups/{id}/ops",
            post(routes::push_ops).get(routes::pull_ops),
        )
        .route(
            "/groups/{id}/invites",
            post(routes::create_invite).delete(routes::delete_invite),
        )
        .route("/invites/{code}/join", post(routes::join));
    Router::new()
        .route("/health", get(routes::health))
        .nest(API_PREFIX, api)
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
        .layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http())
        .with_state(AppState { db })
}

#[cfg(test)]
mod tests;
