mod api;
mod cost;
mod models;
mod parser;
mod watcher;
mod ws;

use axum::{extract::State, response::Json, routing::get, Router};
use models::{AppState, BroadcastTx, SharedState};
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tower_http::cors::{Any, CorsLayer};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("cc_cost_backend=info,info")),
        )
        .init();

    tracing::info!("Scanning Claude project logs...");
    let records = parser::scan_all_records();
    tracing::info!("Loaded {} usage records", records.len());

    let state: SharedState = Arc::new(RwLock::new(AppState { records }));
    let (broadcast_tx, _) = broadcast::channel::<String>(256);
    let broadcast_tx: BroadcastTx = Arc::new(broadcast_tx);

    // File watcher runs in background
    tokio::spawn(watcher::start_watcher(
        Arc::clone(&state),
        Arc::clone(&broadcast_tx),
    ));

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/api/overview",  get(overview_handler))
        .route("/api/sessions",  get(sessions_handler))
        .route("/api/projects",  get(projects_handler))
        .route("/api/rate-card", get(rate_card_handler))
        .route("/ws",            get(ws::ws_handler))
        .with_state((Arc::clone(&state), Arc::clone(&broadcast_tx)))
        .layer(cors);

    let addr = "0.0.0.0:8080";
    tracing::info!("Backend listening on http://{}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn overview_handler(
    State((state, _)): State<(SharedState, BroadcastTx)>,
) -> Json<models::OverviewResponse> {
    let s = state.read().await;
    Json(api::build_overview(&s))
}

async fn sessions_handler(
    State((state, _)): State<(SharedState, BroadcastTx)>,
) -> Json<Vec<models::SessionSummary>> {
    let s = state.read().await;
    Json(api::build_sessions(&s))
}

async fn projects_handler(
    State((state, _)): State<(SharedState, BroadcastTx)>,
) -> Json<Vec<models::ProjectSummary>> {
    let s = state.read().await;
    Json(api::build_projects(&s))
}

async fn rate_card_handler() -> Json<Vec<models::RateEntry>> {
    Json(cost::rate_card())
}
