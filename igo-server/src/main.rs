use std::net::SocketAddr;
use std::path::PathBuf;

use igo_server::{app, ServerConfig};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

fn env_or<T: std::str::FromStr>(key: &str, default: T) -> T {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| "igo_server=info".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let defaults = ServerConfig::default();
    let expiry_days: u64 = env_or("EXPIRY_DAYS", 7);
    let config = ServerConfig {
        max_idle: std::time::Duration::from_secs(expiry_days * 24 * 3600),
        sweep_interval: defaults.sweep_interval,
        board_size: env_or("BOARD_SIZE", defaults.board_size),
        komi: env_or("KOMI", defaults.komi),
        static_dir: std::env::var("STATIC_DIR")
            .map(PathBuf::from)
            .unwrap_or(defaults.static_dir),
    };
    let port: u16 = env_or("PORT", 3030);

    tracing::info!(?config, port, "starting igo");

    let listener = tokio::net::TcpListener::bind(SocketAddr::from(([0, 0, 0, 0], port))).await?;
    axum::serve(listener, app(config)).await?;
    Ok(())
}
