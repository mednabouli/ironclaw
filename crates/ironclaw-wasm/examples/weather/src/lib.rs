// Example WASM plugin: weather.wasm
//
// This is a reference implementation showing how to write an IronClaw
// WASM plugin tool that calls the wttr.in weather API.
//
// To build:
//   cargo component build --release --target wasm32-wasip2
//   cp target/wasm32-wasip2/release/weather.wasm ~/.ironclaw/plugins/
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
//     "temperature_c": "15",
//     "condition": "Partly cloudy",
//     "humidity": "72%",
//     "wind_kmh": "13",
//     "feels_like_c": "14"
//   }
//
// NOTE: This file demonstrates the plugin contract. It cannot be compiled
// to .wasm without the generated WIT bindings (cargo-component).
// See the plugin.json manifest for the tool schema and capabilities.

/// Plugin entry point — returns the tool schema.
///
/// Called once when the plugin is loaded by the host.
fn get_schema() -> ToolSchema {
    ToolSchema {
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

/// Invoke the weather tool.
///
/// Calls `https://wttr.in/{city}?format=j1` via the host HTTP capability,
/// parses the JSON response, and returns a simplified weather summary.
fn invoke(params_json: &str) -> Result<String, String> {
    // Parse input parameters
    let params: serde_json::Value =
        serde_json::from_str(params_json).map_err(|e| format!("Invalid params: {e}"))?;

    let city = params
        .get("city")
        .and_then(|v| v.as_str())
        .ok_or("Missing required parameter: city")?;

    let units = params
        .get("units")
        .and_then(|v| v.as_str())
        .unwrap_or("metric");

    // Build the wttr.in URL
    let unit_param = match units {
        "imperial" => "u",
        _ => "m",
    };
    let url = format!("https://wttr.in/{city}?format=j1&{unit_param}");

    // Call the host HTTP capability
    // In a real WASM component, this calls host::http_fetch()
    let response = host::http_fetch(HttpRequest {
        method: "GET".into(),
        url,
        headers: vec![("User-Agent".into(), "IronClaw-Plugin/0.1".into())],
        body: None,
    })
    .map_err(|e| format!("HTTP request failed: {e}"))?;

    if response.status != 200 {
        return Err(format!(
            "wttr.in returned status {}",
            response.status
        ));
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

    serde_json::to_string(&result).map_err(|e| format!("Serialization error: {e}"))
}

// Placeholder types matching the WIT interface.
// In a real build, these come from generated bindings.
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
        // Provided by the IronClaw host at runtime
        unimplemented!("host import — linked by wasmtime at load time")
    }
}
