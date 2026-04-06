//! REST client example — programmatic HTTP access to IronClaw.
//!
//! Run the server first:
//!   cargo run --bin ironclaw -- start
//!
//! Then in another terminal:
//!   cargo run --example rest_client

use serde::{Deserialize, Serialize};

#[derive(Serialize)]
struct ChatRequest {
    session_id: String,
    message: String,
    stream: bool,
}

#[derive(Deserialize, Debug)]
struct ChatResponse {
    response: String,
    model: String,
    tokens_used: u32,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = reqwest::Client::new();
    let base_url = "http://127.0.0.1:8080";

    // 1. Health check
    println!("📡 Checking health...");
    let health: serde_json::Value = client
        .get(format!("{base_url}/health"))
        .send()
        .await?
        .json()
        .await?;
    println!("Health: {}", serde_json::to_string_pretty(&health)?);

    // 2. Send a chat message
    println!("\n💬 Sending message...");
    let req = ChatRequest {
        session_id: "example-session".into(),
        message: "Hello! What is 2+2?".into(),
        stream: false,
    };

    let resp: ChatResponse = client
        .post(format!("{base_url}/v1/chat"))
        .json(&req)
        .send()
        .await?
        .json()
        .await?;

    println!("Response: {}", resp.response);
    println!("Model:    {}", resp.model);
    println!("Tokens:   {}", resp.tokens_used);

    Ok(())
}
