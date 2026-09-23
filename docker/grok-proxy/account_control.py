#!/usr/bin/env python3
"""Control port for switching the SuperGrok account. Not published to the host."""

from __future__ import annotations

import json
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

from grok_proxy import auth

_lock = threading.Lock()
_login = {
    "status": "idle",
    "verification_url": None,
    "user_code": None,
    "error": None,
}


def _account() -> dict:
    info = auth.status()
    return {
        "logged_in": bool(info.get("logged_in")),
        "account": info.get("account"),
    }


def _start_login() -> dict:
    with _lock:
        if _login["status"] == "waiting":
            return dict(_login)
        _login.update(
            status="waiting",
            verification_url=None,
            user_code=None,
            error=None,
        )

    def run() -> None:
        def echo(line: str = "") -> None:
            text = str(line)
            with _lock:
                if "Open:" in text:
                    _login["verification_url"] = text.split("Open:", 1)[1].strip()
                if "enter code:" in text.lower():
                    _login["user_code"] = text.split(":", 1)[-1].strip()

        try:
            tokens = auth.device_login(open_browser=False, echo=echo)
            claims = auth.jwt_claims(tokens.get("id_token") or "") or auth.jwt_claims(
                tokens["access_token"]
            )
            with _lock:
                _login["status"] = "approved"
                _login["error"] = None
                _login["account"] = claims.get("email") or claims.get("sub")
        except Exception as exc:
            with _lock:
                _login["status"] = "error"
                _login["error"] = str(exc)

    threading.Thread(target=run, daemon=True).start()
    return dict(_login)


class Handler(BaseHTTPRequestHandler):
    def _send(self, code: int, payload: dict) -> None:
        body = json.dumps(payload).encode()
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self) -> None:
        if self.path == "/account":
            self._send(200, _account())
        elif self.path == "/login":
            with _lock:
                payload = dict(_login)
            payload["account"] = _account().get("account")
            self._send(200, payload)
        else:
            self._send(404, {"error": "not found"})

    def do_POST(self) -> None:
        if self.path == "/login":
            self._send(200, _start_login())
        else:
            self._send(404, {"error": "not found"})

    def log_message(self, fmt: str, *args) -> None:
        return


def main() -> None:
    ThreadingHTTPServer(("0.0.0.0", 8586), Handler).serve_forever()


if __name__ == "__main__":
    main()
