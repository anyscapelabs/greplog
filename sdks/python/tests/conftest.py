"""Shared fixtures for Greplog Python SDK tests."""

from __future__ import annotations

import socket
import threading
import time
from typing import Generator, List

import pytest

from greplog.proto.greplog.v1 import events_pb2


class MockAgent:
    def __init__(self) -> None:
        self.port = 0
        self.batches: List[events_pb2.IngestBatch] = []
        self._server: socket.socket | None = None
        self._thread: threading.Thread | None = None
        self._stop = threading.Event()

    def start(self) -> None:
        self._server = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        self._server.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        self._server.bind(("127.0.0.1", 0))
        self.port = self._server.getsockname()[1]
        self._server.listen(5)
        self._server.settimeout(0.5)
        self._thread = threading.Thread(target=self._serve, daemon=True)
        self._thread.start()

    def stop(self) -> None:
        self._stop.set()
        if self._server is not None:
            try:
                self._server.close()
            except OSError:
                pass
        if self._thread is not None:
            self._thread.join(timeout=2)

    def clear(self) -> None:
        self.batches.clear()

    def _serve(self) -> None:
        assert self._server is not None
        while not self._stop.is_set():
            try:
                conn, _addr = self._server.accept()
            except OSError:
                continue
            threading.Thread(target=self._handle_client, args=(conn,), daemon=True).start()

    def _handle_client(self, conn: socket.socket) -> None:
        buffer = b""
        try:
            while not self._stop.is_set():
                try:
                    chunk = conn.recv(65536)
                except OSError:
                    break
                if not chunk:
                    break
                buffer += chunk
                buffer = self._consume_frames(buffer)
        finally:
            try:
                conn.close()
            except OSError:
                pass

    def _consume_frames(self, data: bytes) -> bytes:
        offset = 0
        while offset + 4 <= len(data):
            length = int.from_bytes(data[offset : offset + 4], byteorder="little")
            if offset + 4 + length > len(data):
                break
            frame = data[offset + 4 : offset + 4 + length]
            batch = events_pb2.IngestBatch()
            batch.ParseFromString(frame)
            self.batches.append(batch)
            offset += 4 + length
        return data[offset:]

    def all_logs(self) -> list:
        logs = []
        for batch in self.batches:
            logs.extend(batch.logs)
        return logs


@pytest.fixture(scope="session")
def mock_agent() -> Generator[MockAgent, None, None]:
    agent = MockAgent()
    agent.start()
    yield agent
    agent.stop()


@pytest.fixture(autouse=True)
def reset_sdk(mock_agent: MockAgent) -> Generator[None, None, None]:
    import greplog.hooks as hooks
    import greplog.internal as internal
    import greplog.logging_handler as logging_handler
    import greplog.middleware as middleware
    import logging

    mock_agent.clear()

    if internal.state.transport is not None:
        internal.state.transport.destroy()

    handler = logging_handler.get_greplog_handler()
    if handler is not None:
        logging.getLogger().removeHandler(handler)

    internal.state.initialized = False
    internal.state.config = None
    internal.state.transport = None
    internal.state.event_queue.clear()
    internal.state.batch_seq = 0

    hooks.reset_hook_flags()
    logging_handler.reset_handler_flags()
    middleware.reset_middleware_flags()

    yield

    if internal.state.transport is not None:
        internal.state.transport.destroy()


def wait_for_logs(mock_agent: MockAgent, predicate, timeout: float = 3.0) -> list:
    deadline = time.time() + timeout
    while time.time() < deadline:
        logs = mock_agent.all_logs()
        if predicate(logs):
            return logs
        time.sleep(0.05)
    return mock_agent.all_logs()


@pytest.fixture
def init_kwargs(mock_agent: MockAgent):
    def _kwargs(service_name: str = "test-service") -> dict:
        return {
            "service_name": service_name,
            "tcp_port": mock_agent.port,
            "socket_path": "/nonexistent/greplog.sock",
        }

    return _kwargs
