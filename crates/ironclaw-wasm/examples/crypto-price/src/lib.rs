// Example WASM plugin: crypto-price.wasm
//
// This is a reference implementation showing how to write an IronClaw
// WASM plugin tool that queries CoinGecko for cryptocurrency prices.
//
// To build:
//   cargo component build --release --target wasm32-wasip2
//   cp target/wasm32-wasip2/release/crypto_price.wasm ~/.ironclaw/plugins/
//
// The plugin uses the "http" capability to make outbound requests to:
//   https://api.coingecko.com/api/v3/simple/price
//
// Input parameters (JSON):
//   { "coin": "bitcoin", "currency": "usd" }
//
// Output (JSON):
//   {
//     "coin": "bitcoin",
//     "currency": "usd",
//     "price": 67432.0,
//     "market_cap": 1327000000000,
//     "volume_24h": 28900000000,
//     "change_24h": -1.23
//   }
//
// NOTE: This file demonstrates the plugin contract. It cannot be compiled
// to .wasm without the generated WIT bindings (cargo-component).
// See the plugin.json manifest for the tool schema and capabilities.

/// Plugin entry point — returns the tool schema.
fn get_schema() -> ToolSchema {
    ToolSchema {
        name: "crypto-price".into(),
        description: "Get current cryptocurrency prices from CoinGecko. \
                      Returns price, market cap, 24h volume, and 24h change."
            .into(),
        parameters_json: r#"{
            "type": "object",
            "properties": {
                "coin": {
                    "type": "string",
                    "description": "Cryptocurrency ID (e.g. bitcoin, ethereum, solana)"
                },
                "currency": {
                    "type": "string",
                    "description": "Fiat currency code (default: usd)",
                    "default": "usd"
                }
            },
            "required": ["coin"]
        }"#
        .into(),
    }
}

/// Invoke the crypto-price tool.
///
/// Calls the CoinGecko `/api/v3/simple/price` endpoint via the host HTTP capability.
fn invoke(params_json: &str) -> Result<String, String> {
    // Parse input parameters
    let params: serde_json::Value =
        serde_json::from_str(params_json).map_err(|e| format!("Invalid params: {e}"))?;

    let coin = params
        .get("coin")
        .and_then(|v| v.as_str())
        .ok_or("Missing required parameter: coin")?;

    let currency = params
        .get("currency")
        .and_then(|v| v.as_str())
        .unwrap_or("usd");

    // Build the CoinGecko API URL
    let url = format!(
        "https://api.coingecko.com/api/v3/simple/price?ids={coin}\
         &vs_currencies={currency}\
         &include_market_cap=true\
         &include_24hr_vol=true\
         &include_24hr_change=true"
    );

    // Call the host HTTP capability
    let response = host::http_fetch(HttpRequest {
        method: "GET".into(),
        url,
        headers: vec![
            ("User-Agent".into(), "IronClaw-Plugin/0.1".into()),
            ("Accept".into(), "application/json".into()),
        ],
        body: None,
    })
    .map_err(|e| format!("HTTP request failed: {e}"))?;

    if response.status != 200 {
        return Err(format!("CoinGecko returned status {}", response.status));
    }

    // Parse the CoinGecko JSON response
    let data: serde_json::Value =
        serde_json::from_str(&response.body).map_err(|e| format!("Parse error: {e}"))?;

    let coin_data = data
        .get(coin)
        .ok_or_else(|| format!("Coin '{coin}' not found in CoinGecko response"))?;

    let price = coin_data
        .get(currency)
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);

    let market_cap_key = format!("{currency}_market_cap");
    let volume_key = format!("{currency}_24h_vol");
    let change_key = format!("{currency}_24h_change");

    let result = serde_json::json!({
        "coin": coin,
        "currency": currency,
        "price": price,
        "market_cap": coin_data.get(&market_cap_key).and_then(|v| v.as_f64()).unwrap_or(0.0),
        "volume_24h": coin_data.get(&volume_key).and_then(|v| v.as_f64()).unwrap_or(0.0),
        "change_24h": coin_data.get(&change_key).and_then(|v| v.as_f64()).unwrap_or(0.0),
    });

    serde_json::to_string(&result).map_err(|e| format!("Serialization error: {e}"))
}

// Placeholder types matching the WIT interface.
#[allow(dead_code)]
struct ToolSchema {
    name: String,
    description: String,
    parameters_json: String,
}

#[allow(dead_code)]
struct HttpRequest {
    method: String,
    url: String,
    headers: Vec<(String, String)>,
    body: Option<String>,
}

#[allow(dead_code)]
struct HttpResponse {
    status: u16,
    headers: Vec<(String, String)>,
    body: String,
}

#[allow(dead_code)]
mod host {
    use super::*;
    pub fn http_fetch(_req: HttpRequest) -> Result<HttpResponse, String> {
        unimplemented!("host import — linked by wasmtime at load time")
    }
}
