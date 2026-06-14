#![allow(unused_imports)]

pub mod auth;
pub mod mcp;
pub mod session;
pub mod ws;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{mpsc, oneshot, RwLock};

#[allow(dead_code)]
use crate::relay::session::SessionRegistry;

#[allow(dead_code)]
pub struct ChannelMap {
    pub senders: Vec<mpsc::UnboundedSender<String>>,
}

#[allow(dead_code)]
impl ChannelMap {
    pub fn new() -> Self {
        Self { senders: vec![] }
    }
}

#[allow(dead_code)]
pub struct SharedState {
    pub sessions: SessionRegistry,
    pub agent_broadcast: RwLock<HashMap<String, ChannelMap>>,
    pub pending_mcp: RwLock<HashMap<String, (String, oneshot::Sender<String>)>>,
    pub last_activity: RwLock<HashMap<String, Instant>>,
}

impl SharedState {
    pub fn new() -> Self {
        Self {
            sessions: SessionRegistry::new(),
            agent_broadcast: RwLock::new(HashMap::new()),
            pending_mcp: RwLock::new(HashMap::new()),
            last_activity: RwLock::new(HashMap::new()),
        }
    }
}

use axum::body::Body;
use axum::http::{header, StatusCode, Uri};
use axum::response::Response;

async fn static_handler(uri: Uri) -> Response<Body> {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };

    match crate::web::WebAssets::get(path) {
        Some(content) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, mime.as_ref())
                .body(Body::from(content.data.into_owned()))
                .unwrap()
        }
        None => {
            Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Body::from("Not Found"))
                .unwrap()
        }
    }
}

pub async fn start(
    bind: String,
    _tls_cert: Option<String>,
    _tls_key: Option<String>,
    _dev: bool,
) -> anyhow::Result<()> {
    use axum::routing::get;
    use axum::Router;
    use tower_http::cors::{Any, CorsLayer};

    let state = Arc::new(SharedState::new());

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/ws", get(ws::ws_handler))
        .route("/mcp/sse", get(mcp::sse_handler))
        .route("/mcp/messages", axum::routing::post(mcp::messages_handler))
        .route("/", get(static_handler))
        .route("/session", get(static_handler))
        .route("/style.css", get(static_handler))
        .route("/ws.js", get(static_handler))
        .route("/term.js", get(static_handler))
        .route("/files.js", get(static_handler))
        .route("/session.js", get(static_handler))
        .fallback(get(static_handler))
        .layer(cors)
        .with_state(state.clone());

    let listener = tokio::net::TcpListener::bind(&bind).await?;
    tracing::info!("Relay server listening on {}", bind);

    {
        let state_clone = state.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(tokio::time::Duration::from_secs(300)).await;
                let now = Instant::now();
                let mut to_remove = Vec::new();
                {
                    let activity = state_clone.last_activity.read().await;
                    for (session_id, last) in activity.iter() {
                        if now.duration_since(*last) > tokio::time::Duration::from_secs(1800) {
                            to_remove.push(session_id.clone());
                        }
                    }
                }
                for session_id in to_remove {
                    if !state_clone.sessions.is_temporary(&session_id).await {
                        continue;
                    }
                    tracing::info!("Idle timeout: removing session {}", session_id);
                    {
                        let mut pending = state_clone.pending_mcp.write().await;
                        pending.retain(|_rid, (sid, _tx)| sid != &session_id);
                    }
                    state_clone.sessions.remove(&session_id).await;
                    state_clone.agent_broadcast.write().await.remove(&session_id);
                    state_clone.last_activity.write().await.remove(&session_id);
                }
            }
        });
    }

    axum::serve(listener, app).await?;

    Ok(())
}
