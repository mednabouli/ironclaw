// Example WASM plugin: weather.wasm
//
// This is a fully buildable IronClaw WASM plugin that calls the
// wttr.in weather API.
//
// Prerequisites:
//   rustup target add wasm32-wasip2
//   cargo install cargo-component
//
// Build:
//   cd crates/ironclaw-wasm/examples/weather
//   cargo component build --release
//   cp target/wasm32-wasip2/release/weather_plugin.wasm ~/.ironclaw/plugins/weather/weather.wasm
//   cp plugin.json ~/.ironclaw/plugins/weather/
//
// The plugin uses the "http" capability to make outbound requests to:
//   https://wttr.in/{city}?format=j1
//
// Input parameters (JSON):
//   { "city": "London", "units": "metric" }
//
// Output (JSON):
//   {
//     "city": "London",
//     "temperature": "15",
//     "units": "metric",
//     "condition": "Partly cloudy",
//     "humidity": "72",
//     "wind_kmh": "13",
//     "feels_like": "14"
//   }

// Generate Rust bindings from the IronClaw plugin WIT definition.
// This macro reads the WIT at build time and generates:
//   - `host::http_fetch()` (imported from the IronClaw host)
//   - `host::log()` (imported)
//   - `Guest` trait we must implement (with `get_schema`, `invoke`)
wit_bindgen::generate!({
    world: "plugin",
    path: "../../wit",
});

struct WeatherPlugin;

impl exports::ironclaw::plugin::tool::Guest for WeatherPlugin {
    fn get_schema() -> exports::ironclaw::plugin::tool::ToolSchema {
        exports::ironclaw::plugin::tool::ToolSchema {
            name: "weather".into(),
            description: "Get current weather for a city via wttr.in. \
                          Returns temperature, conditions, humidity, and wind."
                .into(),
            parameters_json: r#"{
                "type": "object",
                "properties": {
                    "city": {
                        "type": "string",
                        "description": "City name (e.g. London, Tokyo, New York)"
                    },
                    "units": {
                        "type": "string",
                        "enum": ["metric", "imperial"],
                        "description": "Temperature units (default: metric)",
                        "default": "metric"
                    }
                },
                "required": ["city"]
            }"#
            .into(),
        }
    }

    fn invoke(params_json: String) -> Result<String, String> {
        use ironclaw::plugin::host;

        // Parse input parameters
        let params: serde_json::Value =
            serde_json::from_str(&params_json).map_err(|e| format!("Invalid params: {e}"))?;

        let city = params
            .get("city")
            .and_then(|v| v.as_str())
            .ok_or("Missing required parameter: city")?;

        let units = params
            .get("units")
            .and_then(|v| v.as_str())
            .unwrap_or("metric");

        // Build the wttr.in URL with unit flag
        let unit_param = match units {
            "imperial" => "u",
            _ => "m",
        };
        let url = format!("https://wttr.in/{city}?format=j1&{unit_param}");

        host::log("debug", &format!("Fetching weather for '{city}' from {url}"));

        // Call the host HTTP capability
        let response = host::http_fetch(&host::HttpRequest {
            method: "GET".into(),
            url,
            headers: vec![("User-Agent".into(), "IronClaw-Plugin/0.1".into())],
            body: None,
        })
        .map_err(|e| format!("HTTP request failed: {e}"))?;

        if response.status != 200 {
            return Err(format!("wttr.in returned status {}", response.status));
        }

        // Parse the wttr.in JSON response
        let data: serde_json::Value =
            serde_json::from_str(&response.body).map_err(|e| format!("Parse error: {e}"))?;

        let current = &data["current_condition"][0];

        let (temp_key, feels_key) = if units == "imperial" {
            ("temp_F", "FeelsLikeF")
        } else {
            ("temp_C", "FeelsLikeC")
        };

        let result = serde_json::json!({
            "city": city,
            "temperature": current[temp_key].as_str().unwrap_or("N/A"),
            "units": units,
            "condition": current["weatherDesc"][0]["value"].as_str().unwrap_or("Unknown"),
            "humidity": current["humidity"].as_str().unwrap_or("N/A"),
            "wind_kmh": current["windspeedKmph"].as_str().unwrap_or("N/A"),
            "feels_like": current[feels_key].as_str().unwrap_or("N/A"),
        });

        host::log("info", &format!("Weather for {city}: {}", result["condition"]));

        serde_json::to_string(&result).map_err(|e| format!("Serialization error: {e}"))
    }
}

export!(WeatherPlugin);

