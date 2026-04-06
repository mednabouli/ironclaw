# IronClaw WASM Plugin Examples

Example plugins demonstrating the IronClaw WASM Component Model plugin system.

## Prerequisites

```bash
rustup target add wasm32-wasip2
cargo install cargo-component
```

## Plugin Structure

Each plugin is a standalone Rust crate that compiles to a WASM component:

```
weather/
├── Cargo.toml       # cargo-component crate (cdylib)
├── plugin.json      # Plugin manifest (name, capabilities, parameters)
└── src/
    └── lib.rs       # Plugin implementation using wit-bindgen
```

## Building

```bash
# Build the weather plugin
cd crates/ironclaw-wasm/examples/weather
cargo component build --release

# Build the crypto-price plugin
cd crates/ironclaw-wasm/examples/crypto-price
cargo component build --release
```

## Installing

Copy the built `.wasm` and `plugin.json` into `~/.ironclaw/plugins/<name>/`:

```bash
# Weather plugin
mkdir -p ~/.ironclaw/plugins/weather
cp target/wasm32-wasip2/release/weather_plugin.wasm ~/.ironclaw/plugins/weather/weather.wasm
cp plugin.json ~/.ironclaw/plugins/weather/

# Or use the CLI installer
ironclaw plugin install https://example.com/weather.wasm
```

## WIT Interface

Plugins implement the `ironclaw:plugin@0.1.0` WIT world defined in
[`../../wit/plugin.wit`](../../wit/plugin.wit):

- **Exports** `tool.get-schema()` — returns the tool's JSON Schema
- **Exports** `tool.invoke(params)` — executes the tool with JSON parameters
- **Imports** `host.http-fetch()` — outbound HTTP (requires `http` capability)
- **Imports** `host.fs-read/write/list()` — sandboxed filesystem (requires `filesystem` capability)
- **Imports** `host.env-get()` — environment variables (requires `env` capability)
- **Imports** `host.log()` — logging (always available)

## Capability Model

Plugins declare required capabilities in `plugin.json`. The IronClaw host only
provides the imports a plugin is allowed to use. Calling an undeclared capability
traps at runtime.

| Capability   | Grants access to               | Constraints                     |
|-------------|--------------------------------|----------------------------------|
| `http`       | `host.http-fetch()`           | `allowed_urls` prefix allowlist  |
| `filesystem` | `host.fs-read/write/list()`   | Sandboxed to `sandbox_dir`       |
| `env`        | `host.env-get()`              | `allowed_env_vars` allowlist     |

## Available Plugins

| Plugin        | Capabilities | API                    | Description                        |
|--------------|-------------|------------------------|-------------------------------------|
| `weather`     | `http`      | wttr.in                | Current weather for any city        |
| `crypto-price`| `http`      | CoinGecko              | Cryptocurrency prices in any fiat   |

## Writing Your Own Plugin

1. Copy one of the example directories as a template
2. Update `Cargo.toml` with your plugin name
3. Update `plugin.json` with capabilities and parameter schema
4. Implement `Guest::get_schema()` and `Guest::invoke()` in `src/lib.rs`
5. Build with `cargo component build --release`
6. Install to `~/.ironclaw/plugins/<name>/`
