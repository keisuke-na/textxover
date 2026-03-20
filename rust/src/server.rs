use axum::{
    extract::State,
    http::StatusCode,
    response::Html,
    routing::{get, post},
    Json, Router,
};
use crossbeam_channel::Sender;
use parking_lot::RwLock;
use std::sync::Arc;
use tower_http::cors::CorsLayer;

use crate::types::{CommentRequest, Config, ConfigRequest, EffectRequest, StatusResponse};

/// Messages sent from HTTP server to the main render loop
#[derive(Debug)]
pub enum ServerMessage {
    Comment(CommentRequest),
    Effect(EffectRequest),
    Config(ConfigRequest),
}

struct AppState {
    sender: Sender<ServerMessage>,
    config: Arc<RwLock<Config>>,
    active_comments: Arc<std::sync::atomic::AtomicU32>,
    active_particles: Arc<std::sync::atomic::AtomicU32>,
}

pub fn start_server(
    port: u16,
    sender: Sender<ServerMessage>,
    config: Arc<RwLock<Config>>,
    active_comments: Arc<std::sync::atomic::AtomicU32>,
    active_particles: Arc<std::sync::atomic::AtomicU32>,
) {
    let state = Arc::new(AppState {
        sender,
        config,
        active_comments,
        active_particles,
    });

    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
        rt.block_on(async {
            let cors = CorsLayer::very_permissive();

            let app = Router::new()
                .route("/comment", post(post_comment))
                .route("/effect", post(post_effect))
                .route("/config", post(post_config))
                .route("/status", get(get_status))
                .route("/ui", get(get_ui))
                .layer(cors)
                .with_state(state);

            let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", port))
                .await
                .expect("Failed to bind server");

            log::info!("HTTP server listening on port {}", port);

            axum::serve(listener, app)
                .await
                .expect("Server failed");
        });
    });
}

async fn post_comment(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CommentRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    log::info!("Comment received: {:?}", req);
    let _ = state.sender.send(ServerMessage::Comment(req));
    (
        StatusCode::OK,
        Json(serde_json::json!({ "id": 0, "status": "queued" })),
    )
}

async fn post_effect(
    State(state): State<Arc<AppState>>,
    Json(req): Json<EffectRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    log::info!("Effect received: {:?}", req);
    let _ = state.sender.send(ServerMessage::Effect(req));
    (
        StatusCode::OK,
        Json(serde_json::json!({ "status": "triggered" })),
    )
}

async fn post_config(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ConfigRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    log::info!("Config update: {:?}", req);
    {
        let mut config = state.config.write();
        config.apply(&req);
    }
    let _ = state.sender.send(ServerMessage::Config(req));
    let config = state.config.read().clone();
    (
        StatusCode::OK,
        Json(serde_json::json!({ "status": "updated", "config": config })),
    )
}

async fn get_ui() -> Html<&'static str> {
    Html(include_str!("../../web/index.html"))
}

async fn get_status(
    State(state): State<Arc<AppState>>,
) -> (StatusCode, Json<StatusResponse>) {
    let config = state.config.read().clone();
    let resp = StatusResponse {
        active_comments: state.active_comments.load(std::sync::atomic::Ordering::Relaxed),
        active_particles: state.active_particles.load(std::sync::atomic::Ordering::Relaxed),
        fps: 60,
        config,
    };
    (StatusCode::OK, Json(resp))
}
