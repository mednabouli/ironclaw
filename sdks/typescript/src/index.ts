/**
 * @ironclaw/client — Typed TypeScript client for the IronClaw AI agent REST + SSE API.
 *
 * Supports both one-shot `/v1/chat` and streaming `/v1/chat/stream` endpoints.
 * Works in Node.js 18+, Next.js (server-side), Deno, Bun, and modern browsers.
 *
 * @example
 * ```ts
 * import { IronClawClient } from "@ironclaw/client";
 *
 * const client = new IronClawClient({ baseUrl: "http://localhost:8080" });
 *
 * // One-shot
 * const res = await client.chat("What is Rust?");
 * console.log(res.response);
 *
 * // Streaming
 * for await (const event of client.stream("Explain closures")) {
 *   if (event.type === "token_delta") process.stdout.write(event.delta);
 * }
 * ```
 *
 * @packageDocumentation
 */

// ── Types ────────────────────────────────────────────────────────────────

/** Configuration for the IronClaw client. */
export interface IronClawConfig {
  /** Base URL of the IronClaw REST API (e.g. "http://localhost:8080"). */
  baseUrl: string;
  /** Optional Bearer token for authenticated endpoints. */
  authToken?: string;
  /** Default session ID. Auto-generated if omitted. */
  sessionId?: string;
  /** Custom fetch implementation (defaults to global fetch). */
  fetch?: typeof globalThis.fetch;
}

/** Request body for POST /v1/chat. */
export interface ChatRequest {
  session_id?: string;
  message: string;
}

/** Response from POST /v1/chat. */
export interface ChatResponse {
  session_id: string;
  response: string;
}

/** Health check response from GET /health. */
export interface HealthResponse {
  status: string;
  version: string;
}

/** SSE event: incremental text token. */
export interface TokenDeltaEvent {
  type: "token_delta";
  delta: string;
}

/** SSE event: tool call starting. */
export interface ToolCallStartEvent {
  type: "tool_call_start";
  id: string;
  name: string;
  arguments: unknown;
}

/** SSE event: tool call finished. */
export interface ToolCallEndEvent {
  type: "tool_call_end";
  id: string;
  result: unknown;
}

/** SSE event: stream complete. */
export interface DoneEvent {
  type: "done";
  usage?: { prompt_tokens: number; completion_tokens: number; total_tokens: number };
}

/** SSE event: error. */
export interface ErrorEvent {
  type: "error";
  message: string;
}

/** Union of all SSE event types. */
export type StreamEvent =
  | TokenDeltaEvent
  | ToolCallStartEvent
  | ToolCallEndEvent
  | DoneEvent
  | ErrorEvent;

// ── Client ───────────────────────────────────────────────────────────────

/**
 * Typed client for the IronClaw AI agent REST + SSE API.
 *
 * All methods throw on network errors or non-2xx responses.
 */
export class IronClawClient {
  private readonly baseUrl: string;
  private readonly authToken?: string;
  private readonly sessionId: string;
  private readonly _fetch: typeof globalThis.fetch;

  constructor(config: IronClawConfig) {
    this.baseUrl = config.baseUrl.replace(/\/+$/, "");
    this.authToken = config.authToken;
    this.sessionId = config.sessionId ?? crypto.randomUUID();
    this._fetch = config.fetch ?? globalThis.fetch.bind(globalThis);
  }

  /** Build common headers for all requests. */
  private headers(): Record<string, string> {
    const h: Record<string, string> = { "Content-Type": "application/json" };
    if (this.authToken) {
      h["Authorization"] = `Bearer ${this.authToken}`;
    }
    return h;
  }

  /**
   * Send a chat message and receive a complete response.
   *
   * @param message  — the user message
   * @param sessionId — optional session override
   * @returns the full chat response
   */
  async chat(message: string, sessionId?: string): Promise<ChatResponse> {
    const body: ChatRequest = {
      session_id: sessionId ?? this.sessionId,
      message,
    };

    const res = await this._fetch(`${this.baseUrl}/v1/chat`, {
      method: "POST",
      headers: this.headers(),
      body: JSON.stringify(body),
    });

    if (!res.ok) {
      const text = await res.text().catch(() => "");
      throw new Error(`IronClaw API error ${res.status}: ${text}`);
    }

    return (await res.json()) as ChatResponse;
  }

  /**
   * Send a chat message and receive a stream of SSE events.
   *
   * Returns an async iterable of typed `StreamEvent` objects.
   * Consumes the stream lazily — break out of the loop to cancel.
   *
   * @param message   — the user message
   * @param sessionId — optional session override
   */
  async *stream(message: string, sessionId?: string): AsyncGenerator<StreamEvent> {
    const body: ChatRequest = {
      session_id: sessionId ?? this.sessionId,
      message,
    };

    const res = await this._fetch(`${this.baseUrl}/v1/chat/stream`, {
      method: "POST",
      headers: { ...this.headers(), Accept: "text/event-stream" },
      body: JSON.stringify(body),
    });

    if (!res.ok) {
      const text = await res.text().catch(() => "");
      throw new Error(`IronClaw API error ${res.status}: ${text}`);
    }

    if (!res.body) {
      throw new Error("No response body — streaming not supported by this runtime");
    }

    const reader = res.body.getReader();
    const decoder = new TextDecoder();
    let buffer = "";

    try {
      while (true) {
        const { done, value } = await reader.read();
        if (done) break;

        buffer += decoder.decode(value, { stream: true });

        // SSE: events are separated by double newlines
        const parts = buffer.split("\n\n");
        buffer = parts.pop() ?? "";

        for (const part of parts) {
          const dataLine = part
            .split("\n")
            .find((line) => line.startsWith("data:"));
          if (!dataLine) continue;

          const json = dataLine.slice(5).trim();
          if (json === "[DONE]") return;

          try {
            const event = JSON.parse(json) as StreamEvent;
            yield event;
            if (event.type === "done") return;
          } catch {
            // Skip malformed SSE data lines
          }
        }
      }
    } finally {
      reader.releaseLock();
    }
  }

  /**
   * Check the health of the IronClaw server.
   *
   * @returns health status and version
   */
  async health(): Promise<HealthResponse> {
    const res = await this._fetch(`${this.baseUrl}/health`, {
      method: "GET",
      headers: this.headers(),
    });

    if (!res.ok) {
      throw new Error(`Health check failed: ${res.status}`);
    }

    return (await res.json()) as HealthResponse;
  }

  /**
   * Fetch raw Prometheus metrics from the /metrics endpoint.
   *
   * @returns Prometheus exposition format text
   */
  async metrics(): Promise<string> {
    const res = await this._fetch(`${this.baseUrl}/metrics`, {
      method: "GET",
    });

    if (!res.ok) {
      throw new Error(`Metrics fetch failed: ${res.status}`);
    }

    return res.text();
  }
}
