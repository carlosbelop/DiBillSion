//! `dibill-server`: reads `DATABASE_URL` (PostgreSQL) and `DIBILL_BIND`
//! (default `127.0.0.1:8080`), migrates the database and serves the API.
//! TLS is terminated in front of it (reverse proxy or platform).

use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();
    let url = std::env::var("DATABASE_URL").map_err(|_| "DATABASE_URL is not set")?;
    let bind = std::env::var("DIBILL_BIND").unwrap_or_else(|_| "127.0.0.1:8080".to_owned());

    let db = dibill_server::pool(url.parse()?, 10)?;
    dibill_server::migrate(&db).await?;
    let listener = TcpListener::bind(&bind).await?;
    tracing::info!(%bind, version = env!("CARGO_PKG_VERSION"), "dibill-server listening");
    axum::serve(listener, dibill_server::app(db))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}
