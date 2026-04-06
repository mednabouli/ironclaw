# @ironclaw/client

Typed TypeScript client for the **IronClaw** AI agent REST + SSE API.

## Install

```bash
npm install @ironclaw/client
```

## Usage

```typescript
import { IronClawClient } from "@ironclaw/client";

const client = new IronClawClient({
  baseUrl: "http://localhost:8080",
  authToken: "my-secret-token", // optional
});

// One-shot chat
const res = await client.chat("What is Rust?");
console.log(res.response);

// Streaming
for await (const event of client.stream("Explain closures in Rust")) {
  if (event.type === "token_delta") {
    process.stdout.write(event.delta);
  }
  if (event.type === "done") {
    console.log("\n[done]", event.usage);
  }
}

// Health check
const health = await client.health();
console.log(health.status); // "ok"
```

## API

### `new IronClawClient(config)`

| Option      | Type     | Required | Description                      |
|-------------|----------|----------|----------------------------------|
| `baseUrl`   | `string` | Yes      | IronClaw REST API base URL       |
| `authToken` | `string` | No       | Bearer token for auth            |
| `sessionId` | `string` | No       | Default session ID               |
| `fetch`     | Function | No       | Custom fetch (e.g. `node-fetch`) |

### `client.chat(message, sessionId?): Promise<ChatResponse>`

Send a message and get a complete response.

### `client.stream(message, sessionId?): AsyncGenerator<StreamEvent>`

Send a message and iterate over SSE events (`token_delta`, `tool_call_start`, `tool_call_end`, `done`, `error`).

### `client.health(): Promise<HealthResponse>`

Check server health.

### `client.metrics(): Promise<string>`

Fetch Prometheus metrics.

## Requirements

- Node.js 18+ / Bun / Deno / modern browsers (needs `fetch` and `ReadableStream`)
- Works with Next.js server components and API routes
