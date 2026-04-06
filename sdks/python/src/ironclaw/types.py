"""Typed data models for the IronClaw API."""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any, Union


@dataclass
class ChatRequest:
    """Request body for POST /v1/chat."""

    message: str
    session_id: str | None = None


@dataclass
class ChatResponse:
    """Response from POST /v1/chat."""

    session_id: str
    response: str


@dataclass
class HealthResponse:
    """Response from GET /health."""

    status: str
    version: str


@dataclass
class TokenDeltaEvent:
    """SSE event: incremental text token."""

    type: str
    delta: str


@dataclass
class ToolCallStartEvent:
    """SSE event: tool call starting."""

    type: str
    id: str
    name: str
    arguments: Any


@dataclass
class ToolCallEndEvent:
    """SSE event: tool call finished."""

    type: str
    id: str
    result: Any


@dataclass
class DoneEvent:
    """SSE event: stream complete."""

    type: str
    usage: dict[str, int] | None = None


@dataclass
class ErrorEvent:
    """SSE event: error."""

    type: str
    message: str


StreamEvent = Union[
    TokenDeltaEvent,
    ToolCallStartEvent,
    ToolCallEndEvent,
    DoneEvent,
    ErrorEvent,
]


def _parse_event(data: dict[str, Any]) -> StreamEvent:
    """Parse a raw SSE JSON dict into a typed event."""
    event_type = data.get("type", "")
    if event_type == "token_delta":
        return TokenDeltaEvent(type=event_type, delta=data.get("delta", ""))
    if event_type == "tool_call_start":
        return ToolCallStartEvent(
            type=event_type,
            id=data.get("id", ""),
            name=data.get("name", ""),
            arguments=data.get("arguments"),
        )
    if event_type == "tool_call_end":
        return ToolCallEndEvent(
            type=event_type,
            id=data.get("id", ""),
            result=data.get("result"),
        )
    if event_type == "done":
        return DoneEvent(type=event_type, usage=data.get("usage"))
    if event_type == "error":
        return ErrorEvent(type=event_type, message=data.get("message", ""))
    # Unknown event — wrap as error
    return ErrorEvent(type="error", message=f"Unknown event type: {event_type}")
