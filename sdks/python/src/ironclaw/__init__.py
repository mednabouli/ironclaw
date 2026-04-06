"""IronClaw Python client — async + sync support for REST and SSE streaming."""

from ironclaw.client import IronClawClient, AsyncIronClawClient
from ironclaw.types import (
    ChatRequest,
    ChatResponse,
    HealthResponse,
    StreamEvent,
    TokenDeltaEvent,
    ToolCallStartEvent,
    ToolCallEndEvent,
    DoneEvent,
    ErrorEvent,
)

__all__ = [
    "IronClawClient",
    "AsyncIronClawClient",
    "ChatRequest",
    "ChatResponse",
    "HealthResponse",
    "StreamEvent",
    "TokenDeltaEvent",
    "ToolCallStartEvent",
    "ToolCallEndEvent",
    "DoneEvent",
    "ErrorEvent",
]

__version__ = "0.1.0"
