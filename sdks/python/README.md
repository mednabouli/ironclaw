# IronClaw Python SDK

Typed Python client for the [IronClaw](https://github.com/mednabouli/ironclaw) AI agent API with sync and async support.

## Install

```bash
pip install ironclaw
```

## Quick Start

### Synchronous

```python
from ironclaw import IronClawClient

with IronClawClient("http://localhost:3000", token="sk-xxx") as client:
    resp = client.chat("What is 2+2?")
    print(resp.response)
```

### Async

```python
import asyncio
from ironclaw import AsyncIronClawClient

async def main():
    async with AsyncIronClawClient("http://localhost:3000") as client:
        async for event in client.stream("Tell me a joke"):
            if event.type == "token_delta":
                print(event.delta, end="", flush=True)
            elif event.type == "done":
                print()

asyncio.run(main())
```

## API Reference

| Method | Description |
|--------|-------------|
| `chat(message, session_id?)` | Send message, get full response |
| `stream(message, session_id?)` | Stream response as SSE events |
| `health()` | Check API health |
| `metrics()` | Get Prometheus metrics text |

## Event Types

| Event | Fields |
|-------|--------|
| `TokenDeltaEvent` | `delta: str` |
| `ToolCallStartEvent` | `id, name, arguments` |
| `ToolCallEndEvent` | `id, result` |
| `DoneEvent` | `usage: dict \| None` |
| `ErrorEvent` | `message: str` |

## Requirements

- Python 3.10+
- httpx
- httpx-sse
