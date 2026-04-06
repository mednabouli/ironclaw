"""Synchronous and asynchronous HTTP clients for the IronClaw API."""

from __future__ import annotations

import json
import uuid
from typing import Any, Generator, AsyncGenerator

import httpx
from httpx_sse import connect_sse, aconnect_sse

from ironclaw.types import (
    ChatRequest,
    ChatResponse,
    HealthResponse,
    StreamEvent,
    _parse_event,
)


class IronClawClient:
    """Synchronous client for the IronClaw REST API.

    Args:
        base_url: API base URL (e.g. ``http://localhost:3000``).
        token: Optional Bearer auth token.
        timeout: Request timeout in seconds.
    """

    def __init__(
        self,
        base_url: str = "http://localhost:3000",
        token: str | None = None,
        timeout: float = 120.0,
    ) -> None:
        headers: dict[str, str] = {}
        if token:
            headers["Authorization"] = f"Bearer {token}"
        self._client = httpx.Client(
            base_url=base_url,
            headers=headers,
            timeout=timeout,
        )

    def close(self) -> None:
        """Close the underlying HTTP client."""
        self._client.close()

    def __enter__(self) -> IronClawClient:
        return self

    def __exit__(self, *_: Any) -> None:
        self.close()

    def chat(
        self,
        message: str,
        session_id: str | None = None,
    ) -> ChatResponse:
        """Send a chat message and wait for the full response.

        Args:
            message: The user message.
            session_id: Optional session identifier. Auto-generated if omitted.

        Returns:
            Parsed ``ChatResponse``.
        """
        payload: dict[str, Any] = {"message": message}
        if session_id:
            payload["session_id"] = session_id
        resp = self._client.post("/v1/chat", json=payload)
        resp.raise_for_status()
        data = resp.json()
        return ChatResponse(
            session_id=data["session_id"],
            response=data["response"],
        )

    def stream(
        self,
        message: str,
        session_id: str | None = None,
    ) -> Generator[StreamEvent, None, None]:
        """Stream a chat response via SSE.

        Args:
            message: The user message.
            session_id: Optional session identifier.

        Yields:
            Typed ``StreamEvent`` objects.
        """
        payload: dict[str, Any] = {"message": message}
        if session_id:
            payload["session_id"] = session_id

        with connect_sse(
            self._client, "POST", "/v1/chat/stream", json=payload
        ) as sse:
            for event in sse.iter_sse():
                if event.data:
                    data = json.loads(event.data)
                    yield _parse_event(data)

    def health(self) -> HealthResponse:
        """Check API health.

        Returns:
            Parsed ``HealthResponse``.
        """
        resp = self._client.get("/health")
        resp.raise_for_status()
        data = resp.json()
        return HealthResponse(status=data["status"], version=data["version"])

    def metrics(self) -> str:
        """Fetch Prometheus metrics.

        Returns:
            Raw metrics text.
        """
        resp = self._client.get("/metrics")
        resp.raise_for_status()
        return resp.text


class AsyncIronClawClient:
    """Asynchronous client for the IronClaw REST API.

    Args:
        base_url: API base URL (e.g. ``http://localhost:3000``).
        token: Optional Bearer auth token.
        timeout: Request timeout in seconds.
    """

    def __init__(
        self,
        base_url: str = "http://localhost:3000",
        token: str | None = None,
        timeout: float = 120.0,
    ) -> None:
        headers: dict[str, str] = {}
        if token:
            headers["Authorization"] = f"Bearer {token}"
        self._client = httpx.AsyncClient(
            base_url=base_url,
            headers=headers,
            timeout=timeout,
        )

    async def close(self) -> None:
        """Close the underlying HTTP client."""
        await self._client.aclose()

    async def __aenter__(self) -> AsyncIronClawClient:
        return self

    async def __aexit__(self, *_: Any) -> None:
        await self.close()

    async def chat(
        self,
        message: str,
        session_id: str | None = None,
    ) -> ChatResponse:
        """Send a chat message and wait for the full response.

        Args:
            message: The user message.
            session_id: Optional session identifier.

        Returns:
            Parsed ``ChatResponse``.
        """
        payload: dict[str, Any] = {"message": message}
        if session_id:
            payload["session_id"] = session_id
        resp = await self._client.post("/v1/chat", json=payload)
        resp.raise_for_status()
        data = resp.json()
        return ChatResponse(
            session_id=data["session_id"],
            response=data["response"],
        )

    async def stream(
        self,
        message: str,
        session_id: str | None = None,
    ) -> AsyncGenerator[StreamEvent, None]:
        """Stream a chat response via SSE.

        Args:
            message: The user message.
            session_id: Optional session identifier.

        Yields:
            Typed ``StreamEvent`` objects.
        """
        payload: dict[str, Any] = {"message": message}
        if session_id:
            payload["session_id"] = session_id

        async with aconnect_sse(
            self._client, "POST", "/v1/chat/stream", json=payload
        ) as sse:
            async for event in sse.aiter_sse():
                if event.data:
                    data = json.loads(event.data)
                    yield _parse_event(data)

    async def health(self) -> HealthResponse:
        """Check API health.

        Returns:
            Parsed ``HealthResponse``.
        """
        resp = await self._client.get("/health")
        resp.raise_for_status()
        data = resp.json()
        return HealthResponse(status=data["status"], version=data["version"])

    async def metrics(self) -> str:
        """Fetch Prometheus metrics.

        Returns:
            Raw metrics text.
        """
        resp = await self._client.get("/metrics")
        resp.raise_for_status()
        return resp.text
