// Example WASM plugin: crypto-price.wasm
//
// This is a fully buildable IronClaw WASM plugin that queries CoinGecko
// for cryptocurrency prices.
//
// Prerequisites:
//   rustup target add wasm32-wasip2
//   cargo install cargo-component
//
// Build:
//   cd crates/ironclaw-wasm/examples/crypto-price
//   cargo component build --release
//   cp target/wasm32-wasip2/release/crypto_price_plugin.wasm ~/.ironclaw/plugins/crypto-price/crypto-price.wasm
//   cp plugin.json ~/.ironclaw/plugins/crypto-price/
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

// Generate Rust bindings from the IronClaw plugin WIT definition.
wit_bindgen::generate!({
    world: "plugin",
    path: "../../wit",
});

struct CryptoPricePlugin;

impl exports::ironclaw::plugin::tool::Guest for CryptoPricePlugin {
    fn get_schema() -> exports::ironclaw::plugin::tool::ToolSchema {
        exports::ironclaw::plugin::tool::ToolSchema {
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

    fn invoke(params_json: String) -> Result<String, String> {
        use ironclaw::plugin::host;

        // Parse input parameters
        let params: serde_json::Value =
            serde_json::from_str(&params_json).map_err(|e| format!("Invalid params: {e}"))?;

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

        host::log("debug", &format!("Fetching price for '{coin}' in {currency}"));

        // Call the host HTTP capability
        let response = host::http_fetch(&host::HttpRequest {
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

        host::log("info", &format!("{coin} = {price} {currency}"));

        serde_json::to_string(&result).map_err(|e| format!("Serialization error: {e}"))
    }
}

export!(CryptoPricePlugin);

