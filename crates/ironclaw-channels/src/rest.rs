use std::{convert::Infallible, net::SocketAddr, sync::Arc, time::Duration};

use async_trait::async_trait;
use axum::{
    extract::{ConnectInfo, Json, Request, State},
    http::{HeaderMap, StatusCode},
    middleware::{self, Next},
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse, Response,
    },
    routing::{get, post},
    Router,
};
use dashmap::DashMap;
use ironclaw_config::RestConfig;
use ironclaw_core::{
    Channel, ChannelError, ChannelId, InboundMessage, MessageHandler, OutboundMessage,
};
use serde::{Deserialize, Serialize};
use tokio_stream::StreamExt;
use tracing::{error, info, warn};

/// Maximum authentication failures per IP before returning 429.
const MAX_AUTH_FAILURES: u32 = 5;
/// Window in which auth failures are counted before reset.
const AUTH_FAILURE_WINDOW: Duration = Duration::from_secs(300);

/// Tracks per-IP authentication failure counts with sliding window.
#[derive(Clone)]
struct AuthRateLimiter {
    /// Map of IP → (failure_count, window_start)
    failures: Arc<DashMap<String, (u32, std::time::Instant)>>,
}

impl AuthRateLimiter {
    fn new() -> Self {
        Self {
            failures: Arc::new(DashMap::new()),
        }
    }

    /// Record an authentication failure for the given IP.
    /// Returns the current failure count.
    fn record_failure(&self, ip: &str) -> u32 {
        let now = std::time::Instant::now();
        let mut entry = self.failures.entry(ip.to_string()).or_insert((0, now));
        let (count, window_start) = entry.value_mut();

        // Reset window if expired
        if now.duration_since(*window_start) > AUTH_FAILURE_WINDOW {
            *count = 0;
            *window_start = now;
        }

        *count += 1;
        *count
    }

    /// Check if the IP has exceeded the failure limit.
    fn is_blocked(&self, ip: &str) -> bool {
        let now = std::time::Instant::now();
        if let Some(entry) = self.failures.get(ip) {
            let (count, window_start) = entry.value();
            if now.duration_since(*window_start) > AUTH_FAILURE_WINDOW {
                return false; // Window expired
            }
            return *count >= MAX_AUTH_FAILURES;
        }
        false
    }
}

#[derive(Clone)]
struct AppState {
    handler: Arc<dyn MessageHandler>,
    auth_token: String,
    auth_limiter: AuthRateLimiter,
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
/// Returns 429 Too Many Requests after [`MAX_AUTH_FAILURES`] failed attempts
/// from the same IP within [`AUTH_FAILURE_WINDOW`].
async fn require_auth(
    State(state): State<AppState>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Response {
    if state.auth_token.is_empty() {
        return next.run(request).await;
    }

    let client_ip = connect_info
        .map(|ci| ci.0.ip().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    // Check if this IP is currently rate-limited
    if state.auth_limiter.is_blocked(&client_ip) {
        warn!(ip = %client_ip, "REST: rate-limited auth attempt (too many failures)");
        return (
            StatusCode::TOO_MANY_REQUESTS,
            "Too many authentication failures. Try again later.",
        )
            .into_response();
    }

    let expected = format!("Bearer {}", state.auth_token);
    let provided = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if provided != expected {
        let failures = state.auth_limiter.record_failure(&client_ip);
        warn!(ip = %client_ip, failures, "REST: rejected request with invalid or missing Authorization header");
        return (StatusCode::UNAUTHORIZED, "Unauthorized").into_response();
    }

    next.run(request).await
}

async fn chat_handler(State(state): State<AppState>, Json(body): Json<ChatRequest>) -> Response {
    let session_id = body
        .session_id
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let inbound = InboundMessage::builder(ChannelId::Rest(session_id.clone()), body.message)
        .session_id(session_id.clone())
        .build();

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

/// Prometheus metrics endpoint — returns text exposition format.
async fn metrics_handler() -> Response {
    let body = crate::metrics::render();
    (
        StatusCode::OK,
        [("content-type", "text/plain; version=0.0.4; charset=utf-8")],
        body,
    )
        .into_response()
}

/// SSE streaming chat endpoint.
/// Calls `handler.handle_stream()` and converts each `StreamEvent` into an
/// SSE `Event` with event type and JSON data.
async fn stream_chat_handler(
    State(state): State<AppState>,
    Json(body): Json<ChatRequest>,
) -> Response {
    let session_id = body
        .session_id
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let inbound = InboundMessage::builder(ChannelId::Rest(session_id.clone()), body.message)
        .session_id(session_id.clone())
        .build();

    let event_stream = match state.handler.handle_stream(inbound).await {
        Ok(s) => s,
        Err(e) => {
            error!(error = %e, "handle_stream error");
            return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();
        }
    };

    let sse_stream = event_stream.map(|result| -> Result<Event, Infallible> {
        match result {
            Ok(event) => {
                let event_type = match &event {
                    ironclaw_core::StreamEvent::TokenDelta { .. } => "token_delta",
                    ironclaw_core::StreamEvent::ToolCallStart { .. } => "tool_call_start",
                    ironclaw_core::StreamEvent::ToolCallEnd { .. } => "tool_call_end",
                    ironclaw_core::StreamEvent::Done { .. } => "done",
                    ironclaw_core::StreamEvent::Error { .. } => "error",
                    _ => "unknown",
                };
                match serde_json::to_string(&event) {
                    Ok(json) => Ok(Event::default().event(event_type).data(json)),
                    Err(e) => {
                        error!(error = %e, event_type, "SSE event serialization failed");
                        let err_json = serde_json::json!({
                            "type": "error",
                            "message": format!("Internal serialization error: {e}")
                        });
                        Ok(Event::default().event("error").data(err_json.to_string()))
                    }
                }
            }
            Err(e) => {
                let json = serde_json::json!({ "type": "error", "message": e.to_string() });
                Ok(Event::default().event("error").data(json.to_string()))
            }
        }
    });

    Sse::new(sse_stream)
        .keep_alive(KeepAlive::default())
        .into_response()
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

    async fn start(&self, handler: Arc<dyn MessageHandler>) -> Result<(), ChannelError> {
        (async {
            let state = AppState {
                handler,
                auth_token: self.config.auth_token.clone(),
                auth_limiter: AuthRateLimiter::new(),
            };

            let app = Router::new()
                // Protected routes — auth middleware applied via route_layer (only these routes)
                .route("/v1/chat", post(chat_handler))
                .route("/v1/chat/stream", post(stream_chat_handler))
                .route_layer(middleware::from_fn_with_state(state.clone(), require_auth))
                // Public routes — no auth required
                .route("/health", get(health_handler))
                .route("/metrics", get(metrics_handler))
                .with_state(state);

            let addr: SocketAddr = format!("{}:{}", self.config.host, self.config.port)
                .parse()
                .map_err(|e| anyhow::anyhow!("Invalid REST addr: {e}"))?;

            info!(addr = %addr, "REST channel listening");

            let listener = tokio::net::TcpListener::bind(addr).await?;
            axum::serve(
                listener,
                app.into_make_service_with_connect_info::<SocketAddr>(),
            )
            .await?;
            Ok::<(), anyhow::Error>(())
        })
        .await
        .map_err(Into::into)
    }

    async fn send(&self, _to: &ChannelId, _message: OutboundMessage) -> Result<(), ChannelError> {
        Ok(())
    }
    async fn stop(&self) -> Result<(), ChannelError> {
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
            auth_limiter: AuthRateLimiter::new(),
        };
        Router::new()
            .route("/v1/chat", post(chat_handler))
            .route("/v1/chat/stream", post(stream_chat_handler))
            .route_layer(middleware::from_fn_with_state(state.clone(), require_auth))
            .route("/health", get(health_handler))
            .route("/metrics", get(metrics_handler))
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

    #[tokio::test]
    async fn stream_rejects_missing_token() {
        let app = make_app("secret");
        let req = HttpRequest::builder()
            .method("POST")
            .uri("/v1/chat/stream")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"message":"hi"}"#))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn stream_returns_sse_content_type() {
        // With no auth, the endpoint should accept the request and start streaming.
        // NoopHandler's default handle_stream calls handle() which errors,
        // so we'll get a 500, but with auth disabled it won't be 401.
        let state = AppState {
            handler: Arc::new(StreamTestHandler),
            auth_token: String::new(),
            auth_limiter: AuthRateLimiter::new(),
        };
        let app = Router::new()
            .route("/v1/chat/stream", post(stream_chat_handler))
            .with_state(state);

        let req = HttpRequest::builder()
            .method("POST")
            .uri("/v1/chat/stream")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"message":"hi"}"#))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let ct = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        assert!(
            ct.contains("text/event-stream"),
            "Expected SSE content-type, got: {ct}"
        );
    }

    /// A handler that returns a short stream for testing.
    struct StreamTestHandler;

    #[async_trait]
    impl MessageHandler for StreamTestHandler {
        async fn handle(
            &self,
            _msg: InboundMessage,
        ) -> Result<Option<OutboundMessage>, ironclaw_core::HandlerError> {
            Ok(None)
        }

        async fn handle_stream(
            &self,
            _msg: InboundMessage,
        ) -> Result<ironclaw_core::BoxStream<ironclaw_core::StreamEvent>, ironclaw_core::HandlerError>
        {
            let events: Vec<anyhow::Result<ironclaw_core::StreamEvent>> = vec![
                Ok(ironclaw_core::StreamEvent::TokenDelta {
                    delta: "Hello".into(),
                }),
                Ok(ironclaw_core::StreamEvent::Done { usage: None }),
            ];
            Ok(Box::pin(futures::stream::iter(events)))
        }
    }

    #[tokio::test]
    async fn stream_emits_well_formed_sse_events() {
        let state = AppState {
            handler: Arc::new(StreamTestHandler),
            auth_token: String::new(),
            auth_limiter: AuthRateLimiter::new(),
        };
        let app = Router::new()
            .route("/v1/chat/stream", post(stream_chat_handler))
            .with_state(state);

        let req = HttpRequest::builder()
            .method("POST")
            .uri("/v1/chat/stream")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"message":"hi"}"#))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body_bytes = axum::body::to_bytes(resp.into_body(), 1024 * 64)
            .await
            .unwrap();
        let body_str = String::from_utf8_lossy(&body_bytes);

        // SSE format: "event: <type>\ndata: <json>\n\n"
        // Parse out the events (skip keep-alive comments)
        let mut events: Vec<(String, serde_json::Value)> = Vec::new();
        let mut current_event = String::new();
        let mut current_data = String::new();
        for line in body_str.lines() {
            if let Some(ev) = line.strip_prefix("event: ") {
                current_event = ev.to_string();
            } else if let Some(d) = line.strip_prefix("data: ") {
                current_data = d.to_string();
            } else if line.is_empty() && !current_event.is_empty() {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&current_data) {
                    events.push((current_event.clone(), val));
                }
                current_event.clear();
                current_data.clear();
            }
        }

        assert!(
            events.len() >= 2,
            "Expected ≥2 SSE events, got {}",
            events.len()
        );

        // First event: token_delta with "Hello"
        assert_eq!(events[0].0, "token_delta");
        assert_eq!(events[0].1["type"], "token_delta");
        assert_eq!(events[0].1["delta"], "Hello");

        // Second event: done
        assert_eq!(events[1].0, "done");
        assert_eq!(events[1].1["type"], "done");
        assert!(
            events[1].1["usage"].is_null(),
            "streaming usage should be null"
        );
    }

    #[test]
    fn auth_rate_limiter_blocks_after_max_failures() {
        let limiter = AuthRateLimiter::new();
        let ip = "192.168.1.100";

        // Should not be blocked initially
        assert!(!limiter.is_blocked(ip));

        // Record MAX_AUTH_FAILURES failures
        for _ in 0..MAX_AUTH_FAILURES {
            limiter.record_failure(ip);
        }

        // Should now be blocked
        assert!(limiter.is_blocked(ip));

        // Different IP should not be affected
        assert!(!limiter.is_blocked("10.0.0.1"));
    }

    #[test]
    fn auth_rate_limiter_allows_below_threshold() {
        let limiter = AuthRateLimiter::new();
        let ip = "192.168.1.200";

        for _ in 0..(MAX_AUTH_FAILURES - 1) {
            limiter.record_failure(ip);
        }

        // One below the limit — should still be allowed
        assert!(!limiter.is_blocked(ip));
    }
}
