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

use crate::types::{
    CommentRequest, Config, ConfigRequest, EffectRequest, PollChoiceResult, PollStartRequest,
    PollState, StatusResponse,
};

/// Messages sent from HTTP server to the main render loop
#[derive(Debug)]
pub enum ServerMessage {
    Comment(CommentRequest),
    Effect(EffectRequest),
    Config(ConfigRequest),
    PollStart(PollStartRequest),
    PollStop,
}

pub struct AppState {
    sender: Sender<ServerMessage>,
    config: Arc<RwLock<Config>>,
    active_comments: Arc<std::sync::atomic::AtomicU32>,
    active_particles: Arc<std::sync::atomic::AtomicU32>,
    pub poll: Arc<RwLock<PollState>>,
}

/// Check if request is from localhost (no CF-Connecting-IP header = direct access)
fn is_host(headers: &axum::http::HeaderMap) -> bool {
    !headers.contains_key("cf-connecting-ip")
}

pub fn start_server(
    port: u16,
    sender: Sender<ServerMessage>,
    config: Arc<RwLock<Config>>,
    active_comments: Arc<std::sync::atomic::AtomicU32>,
    active_particles: Arc<std::sync::atomic::AtomicU32>,
    poll: Arc<RwLock<PollState>>,
) {
    let state = Arc::new(AppState {
        sender,
        config,
        active_comments,
        active_particles,
        poll,
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
                .route("/is-host", get(get_is_host))
                .route("/poll/start", post(post_poll_start))
                .route("/poll/stop", post(post_poll_stop))
                .route("/poll/status", get(get_poll_status))
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
    // Check if this comment is a poll vote
    {
        let mut poll = state.poll.write();
        if poll.active {
            let text = req.text.trim().to_uppercase();
            if let Some(choice) = poll.choices.iter_mut().find(|c| c.key.to_uppercase() == text) {
                choice.count += 1;
                log::info!("Poll vote: {} ({})", choice.key, choice.count);
            }
        }
    }

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

async fn get_is_host(
    headers: axum::http::HeaderMap,
) -> (StatusCode, Json<serde_json::Value>) {
    (StatusCode::OK, Json(serde_json::json!({ "host": is_host(&headers) })))
}

async fn post_poll_start(
    headers: axum::http::HeaderMap,
    State(state): State<Arc<AppState>>,
    Json(req): Json<PollStartRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    if !is_host(&headers) {
        return (StatusCode::FORBIDDEN, Json(serde_json::json!({"error": "forbidden"})));
    }

    {
        let mut poll = state.poll.write();
        poll.active = true;
        poll.question = req.question.clone();
        poll.choices = req
            .choices
            .iter()
            .map(|c| PollChoiceResult {
                key: c.key.clone(),
                label: c.label.clone(),
                count: 0,
            })
            .collect();
    }

    let _ = state.sender.send(ServerMessage::PollStart(req));
    log::info!("Poll started");
    (
        StatusCode::OK,
        Json(serde_json::json!({ "status": "started" })),
    )
}

async fn post_poll_stop(
    headers: axum::http::HeaderMap,
    State(state): State<Arc<AppState>>,
) -> (StatusCode, Json<serde_json::Value>) {
    if !is_host(&headers) {
        return (StatusCode::FORBIDDEN, Json(serde_json::json!({"error": "forbidden"})));
    }

    {
        let mut poll = state.poll.write();
        poll.active = false;
    }

    let _ = state.sender.send(ServerMessage::PollStop);
    log::info!("Poll stopped");
    (
        StatusCode::OK,
        Json(serde_json::json!({ "status": "stopped" })),
    )
}

async fn get_poll_status(
    State(state): State<Arc<AppState>>,
) -> (StatusCode, Json<PollState>) {
    let poll = state.poll.read().clone();
    (StatusCode::OK, Json(poll))
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
