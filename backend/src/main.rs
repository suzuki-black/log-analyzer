mod config;
mod db;
mod error;
mod models;
mod routes;
mod ingest;

use axum::{Router, routing::{get, post, delete}, extract::DefaultBodyLimit};
use tower_http::cors::{CorsLayer, Any};
use std::sync::Arc;
use tokio::sync::RwLock;
use std::collections::HashMap;
use tokio::sync::broadcast;

pub struct AppState {
    pub pool: sqlx::MySqlPool,
    pub config: config::Config,
    pub jobs: RwLock<HashMap<String, models::Job>>,
    pub senders: RwLock<HashMap<String, broadcast::Sender<models::SseEvent>>>,
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    let config = config::Config::from_env();
    let pool = db::create_pool(&config.database_url).await;
    db::init(&pool).await;

    let state = Arc::new(AppState {
        pool,
        config,
        jobs: RwLock::new(HashMap::new()),
        senders: RwLock::new(HashMap::new()),
    });

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/api/upload", post(routes::upload::upload_file)
            .layer(DefaultBodyLimit::max(500 * 1024 * 1024)))
        .route("/api/jobs", post(routes::jobs::create_job).get(routes::jobs::list_jobs))
        .route("/api/jobs/:id", get(routes::jobs::get_job))
        .route("/api/jobs/:id/progress", get(routes::sse::sse_handler))
        .route("/api/tables", get(routes::jobs::list_tables))
        .route("/api/tables/:name/truncate", post(routes::jobs::truncate_table))
        .route("/api/tables/:name", delete(routes::jobs::drop_table))
        .layer(cors)
        .with_state(state);

    let addr = "0.0.0.0:8080";
    println!("Listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
