"""Non-blocking dual-transport client for the local Greplog agent."""

from __future__ import annotations

import os
import queue
import socket
import sys
import threading
import time
from typing import Optional

from greplog.serialize import encode_length_prefixed_frame

_warning_shown = False


def _show_warning() -> None:
    global _warning_shown
    if _warning_shown:
        return
    _warning_shown = True
    try:
        sys.stderr.write("[Greplog] Agent not found. Run 'greplog dev' to capture logs.\n")
    except Exception:
        pass


class Transport:
    def __init__(
        self,
        socket_path: Optional[str] = None,
        tcp_host: str = "127.0.0.1",
        tcp_port: int = 4318,
    ) -> None:
        self._socket_path = socket_path or ".greplog/greplog.sock"
        self._tcp_host = tcp_host
        self._tcp_port = tcp_port
        self._sock: Optional[socket.socket] = None
        self._connecting = False
        self._destroyed = False
        self._write_buffer: queue.SimpleQueue[bytes] = queue.SimpleQueue()
        self._pending_frames: list[bytes] = []
        self._lock = threading.Lock()
        self._worker = threading.Thread(target=self._run, name="greplog-transport", daemon=True)
        self._worker.start()

    def connect(self) -> None:
        # Connection attempts are driven by the background worker.
        pass

    def send(self, payload: bytes) -> None:
        if self._destroyed:
            return
        try:
            frame = encode_length_prefixed_frame(payload)
            self._write_buffer.put_nowait(frame)
        except Exception:
            pass

    def destroy(self) -> None:
        self._destroyed = True
        try:
            self._write_buffer.put_nowait(b"")
        except Exception:
            pass
        if self._sock is not None:
            try:
                self._sock.close()
            except Exception:
                pass
            self._sock = None

    def _run(self) -> None:
        while not self._destroyed:
            try:
                frame = self._write_buffer.get(timeout=0.5)
            except queue.Empty:
                continue

            if self._destroyed:
                break
            if not frame:
                continue

            if self._sock is None:
                self._pending_frames.append(frame)
                if len(self._pending_frames) > 1000:
                    self._pending_frames.pop(0)
                _show_warning()
                self._ensure_connected()
                continue

            try:
                self._sock.sendall(frame)
            except Exception:
                self._pending_frames.append(frame)
                if len(self._pending_frames) > 1000:
                    self._pending_frames.pop(0)
                self._close_socket()
                _show_warning()
                time.sleep(5)

    def _ensure_connected(self) -> None:
        with self._lock:
            if self._destroyed or self._sock is not None or self._connecting:
                return
            self._connecting = True

        try:
            if os.name == "nt":
                sock = self._connect_tcp()
            else:
                sock = self._connect_uds()
                if sock is None:
                    sock = self._connect_tcp()

            if sock is not None:
                with self._lock:
                    self._sock = sock
                    frames = list(self._pending_frames)
                    self._pending_frames.clear()
                for pending in frames:
                    try:
                        sock.sendall(pending)
                    except Exception:
                        with self._lock:
                            self._pending_frames.extend(frames)
                        self._close_socket()
                        break
        finally:
            with self._lock:
                self._connecting = False

    def _connect_uds(self) -> Optional[socket.socket]:
        try:
            sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
            sock.settimeout(5)
            sock.connect(self._socket_path)
            return sock
        except Exception:
            try:
                sock.close()
            except Exception:
                pass
            return None

    def _connect_tcp(self) -> Optional[socket.socket]:
        try:
            sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
            sock.settimeout(5)
            sock.connect((self._tcp_host, self._tcp_port))
            return sock
        except Exception:
            try:
                sock.close()
            except Exception:
                pass
            return None

    def _close_socket(self) -> None:
        with self._lock:
            if self._sock is not None:
                try:
                    self._sock.close()
                except Exception:
                    pass
                self._sock = None
