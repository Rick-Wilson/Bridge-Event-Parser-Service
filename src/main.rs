mod adapters;
mod api;
mod builder;
mod error;
mod responses;
mod schema;
mod sessions;
mod types;
mod upload_helpers;

use axum::{
    extract::DefaultBodyLimit,
    http::{HeaderValue, Method},
    routing::{get, post},
    Router,
};
use sessions::SessionStore;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;
use tower_http::cors::{AllowOrigin, CorsLayer};

pub struct AppState {
    pub sessions: SessionStore,
    pub started_at: Instant,
    pub version: &'static str,
}

#[tokio::main]
async fn main() {
    let _ = dotenvy::dotenv();

    let log_level = std::env::var("LOG_LEVEL").unwrap_or_else(|_| "info".into());
    let log_format = std::env::var("LOG_FORMAT").unwrap_or_else(|_| "json".into());
    init_logging(&log_level, &log_format);

    let host = std::env::var("HOST").unwrap_or_else(|_| "0.0.0.0".into());
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3001);

    let state = Arc::new(AppState {
        sessions: SessionStore::new(),
        started_at: Instant::now(),
        version: env!("CARGO_PKG_VERSION"),
    });

    // Background session sweep every 5 minutes.
    let sweep_state = Arc::clone(&state);
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(300));
        loop {
            interval.tick().await;
            sweep_state.sessions.sweep();
        }
    });

    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::list([
            "https://bridge-classroom.com"
                .parse::<HeaderValue>()
                .unwrap(),
            "https://www.bridge-classroom.com"
                .parse::<HeaderValue>()
                .unwrap(),
            "https://bridge-classroom.org"
                .parse::<HeaderValue>()
                .unwrap(),
            "https://www.bridge-classroom.org"
                .parse::<HeaderValue>()
                .unwrap(),
            "https://game-analysis.bridge-classroom.com"
                .parse::<HeaderValue>()
                .unwrap(),
            "https://game-analysis.bridge-classroom.org"
                .parse::<HeaderValue>()
                .unwrap(),
            "https://bridge-craftwork.com"
                .parse::<HeaderValue>()
                .unwrap(),
            "http://localhost:3001".parse::<HeaderValue>().unwrap(),
            "http://localhost:5173".parse::<HeaderValue>().unwrap(),
            "http://127.0.0.1:5173".parse::<HeaderValue>().unwrap(),
        ]))
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers(tower_http::cors::Any);

    let app = Router::new()
        .route("/api/upload", post(api::upload_files))
        .route("/api/upload-normalized", post(api::upload_normalized))
        .route("/api/normalized", get(api::get_normalized))
        .route("/healthz", get(api::healthz))
        .layer(cors)
        .layer(DefaultBodyLimit::max(20 * 1024 * 1024))
        .with_state(state);

    let addr = SocketAddr::new(host.parse().expect("Invalid HOST"), port);
    tracing::info!("bridge-event-parser-service listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("Failed to bind");
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .expect("Server error");
}

fn init_logging(level: &str, format: &str) {
    use tracing_subscriber::{fmt, EnvFilter};
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(level));
    if format == "json" {
        tracing_subscriber::fmt()
            .json()
            .with_env_filter(filter)
            .init();
    } else {
        fmt().with_env_filter(filter).init();
    }
}
