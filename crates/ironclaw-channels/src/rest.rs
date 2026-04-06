use std::{net::SocketAddr, sync::Arc};

use async_trait::async_trait;
use axum::{
    extract::{Json, Request, State},
    http::{HeaderMap, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use ironclaw_config::RestConfig;
use ironclaw_core::{Channel, ChannelId, InboundMessage, MessageHandler, OutboundMessage};
use serde::{Deserialize, Serialize};
use tracing::{error, info, warn};

#[derive(Clone)]
struct AppState {
    handler: Arc<dyn MessageHandler>,
    auth_token: String,
}

#[derive(Deserialize)]
struct ChatRequest {
    session_id: Option<String>,
    message: String,
}

#[derive(Serialize)]
struct ChatResponse {
    session_id: String,
    response: String,
}

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    version: &'static str,
}

/// Middleware that enforces Bearer token auth when `auth_token` is configured.
/// Passes through without checking when `auth_token` is empty.
async fn require_auth(
    State(state): State<AppState>,
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Response {
    if state.auth_token.is_empty() {
        return next.run(request).await;
    }

    let expected = format!("Bearer {}", state.auth_token);
    let provided = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if provided != expected {
        warn!("REST: rejected request with invalid or missing Authorization header");
        return (StatusCode::UNAUTHORIZED, "Unauthorized").into_response();
    }

    next.run(request).await
}

async fn chat_handler(State(state): State<AppState>, Json(body): Json<ChatRequest>) -> Response {
    let session_id = body
        .session_id
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let inbound = InboundMessage {
        id: uuid::Uuid::new_v4().to_string(),
        channel: ChannelId::Rest(session_id.clone()),
        session_id: session_id.clone(),
        content: body.message,
        author: None,
        timestamp: chrono::Utc::now(),
    };

    match state.handler.handle(inbound).await {
        Ok(Some(out)) => Json(ChatResponse {
            session_id,
            response: out.as_str().to_string(),
        })
        .into_response(),
        Ok(None) => (StatusCode::NO_CONTENT, "").into_response(),
        Err(e) => {
            error!(error = %e, "Handler error");
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}

async fn health_handler() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
    })
}

/// REST channel: serves `/v1/chat` (POST, auth-protected) and `/health` (GET, public).
pub struct RestChannel {
    config: RestConfig,
}

impl RestChannel {
    /// Create a new REST channel from the given config.
    pub fn new(config: RestConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Channel for RestChannel {
    fn name(&self) -> &'static str {
        "rest"
    }

    async fn start(&self, handler: Arc<dyn MessageHandler>) -> anyhow::Result<()> {
        let state = AppState {
            handler,
            auth_token: self.config.auth_token.clone(),
        };

        let app = Router::new()
            // Protected routes — auth middleware applied via route_layer (only these routes)
            .route("/v1/chat", post(chat_handler))
            .route_layer(middleware::from_fn_with_state(state.clone(), require_auth))
            // Public route — no auth required
            .route("/health", get(health_handler))
            .with_state(state);

        let addr: SocketAddr = format!("{}:{}", self.config.host, self.config.port)
            .parse()
            .map_err(|e| anyhow::anyhow!("Invalid REST addr: {e}"))?;

        info!(addr = %addr, "REST channel listening");

        let listener = tokio::net::TcpListener::bind(addr).await?;
        axum::serve(listener, app).await?;
        Ok(())
    }

    async fn send(&self, _to: &ChannelId, _message: OutboundMessage) -> anyhow::Result<()> {
        Ok(())
    }
    async fn stop(&self) -> anyhow::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use axum::{body::Body, http::Request as HttpRequest};
    use tower::util::ServiceExt;

    use super::*;

    fn make_app(auth_token: &str) -> Router {
        let state = AppState {
            handler: Arc::new(crate::tests::NoopHandler),
            auth_token: auth_token.to_string(),
        };
        Router::new()
            .route("/v1/chat", post(chat_handler))
            .route_layer(middleware::from_fn_with_state(state.clone(), require_auth))
            .route("/health", get(health_handler))
            .with_state(state)
    }

    #[tokio::test]
    async fn health_is_public() {
        let app = make_app("secret");
        let req = HttpRequest::builder()
            .uri("/health")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn chat_rejects_missing_token() {
        let app = make_app("secret");
        let req = HttpRequest::builder()
            .method("POST")
            .uri("/v1/chat")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"message":"hi"}"#))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn chat_rejects_wrong_token() {
        let app = make_app("secret");
        let req = HttpRequest::builder()
            .method("POST")
            .uri("/v1/chat")
            .header("content-type", "application/json")
            .header("authorization", "Bearer wrong")
            .body(Body::from(r#"{"message":"hi"}"#))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn chat_passes_with_no_auth_configured() {
        // When auth_token is empty, all requests pass through
        let app = make_app("");
        let req = HttpRequest::builder()
            .method("POST")
            .uri("/v1/chat")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"message":"hi"}"#))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        // Handler will return 500 (NoopHandler returns error), but NOT 401
        assert_ne!(resp.status(), StatusCode::UNAUTHORIZED);
    }
}
