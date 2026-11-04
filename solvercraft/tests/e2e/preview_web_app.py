"""Load actual Trunk Gridcraft assets in isolated Chrome and capture its canvas.

Uses only WebDriver navigation, DOM status, pointer/keyboard actions and screenshot
commands. No injected JavaScript, production hooks or user browser profiles.
"""
from __future__ import annotations

import argparse
import base64
from functools import partial
import hashlib
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import socket
import subprocess
import tempfile
import threading
import time
import urllib.error
import urllib.request


class QuietFiles(SimpleHTTPRequestHandler):
    def log_message(self, *_args: object) -> None:
        pass


def request(base: str, method: str, path: str, body: object = None) -> object:
    data = json.dumps(body).encode() if body is not None else None
    req = urllib.request.Request(base + path, data=data, method=method, headers={"Content-Type": "application/json"})
    try:
        with urllib.request.urlopen(req, timeout=60) as response:
            raw = response.read(24 * 1024 * 1024 + 1)
    except urllib.error.HTTPError as error:
        if error.code == 404:
            raise
        detail = error.read(16 * 1024).decode(errors="replace")
        raise RuntimeError(f"WebDriver {method} {path} failed HTTP {error.code}: {detail}") from error
    if len(raw) > 24 * 1024 * 1024:
        raise ValueError("WebDriver response exceeds budget")
    return json.loads(raw)["value"]


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--assets", type=Path, required=True)
    parser.add_argument("--driver", type=Path, required=True)
    parser.add_argument("--browser", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--click", action="append", default=[], help="CSS pixel x,y, interpreted in sequence")
    args = parser.parse_args()
    if not (args.assets / "index.html").is_file():
        parser.error("Actual Trunk dist index.html required")
    run_dir = Path(tempfile.mkdtemp(prefix="solvercraft-app-"))
    server = ThreadingHTTPServer(("127.0.0.1", 0), partial(QuietFiles, directory=str(args.assets.resolve())))
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    with socket.socket() as listener:
        listener.bind(("127.0.0.1", 0))
        port = listener.getsockname()[1]
    endpoint = f"http://127.0.0.1:{port}"
    session = None
    record: dict[str, object] = {"actual_app": True, "assets": str(args.assets.resolve()), "actions": args.click}
    with (run_dir / "driver.log").open("w", encoding="utf-8") as log:
        environment = os.environ.copy()
        environment["TMP"] = str(run_dir)
        environment["TEMP"] = str(run_dir)
        driver = subprocess.Popen([str(args.driver), f"--port={port}"], stdout=log, stderr=subprocess.STDOUT, env=environment)
        try:
            deadline = time.monotonic() + 20
            while True:
                try:
                    request(endpoint, "GET", "/status")
                    break
                except (OSError, ValueError):
                    if time.monotonic() >= deadline:
                        raise
                    time.sleep(0.1)
            created = request(endpoint, "POST", "/session", {"capabilities": {"alwaysMatch": {
                "browserName": "chrome", "unhandledPromptBehavior": "dismiss",
                "goog:chromeOptions": {"binary": str(args.browser), "args": ["--headless=new", "--window-size=1600,1100", f"--user-data-dir={run_dir / 'profile'}"]},
                "goog:loggingPrefs": {"browser": "ALL"}
            }}})
            session = created["sessionId"]
            record["browser_capabilities"] = created["capabilities"]
            prefix = f"/session/{session}"
            page = f"http://127.0.0.1:{server.server_port}/index.html"
            record["url"] = page
            request(endpoint, "POST", prefix + "/url", {"url": page})
            deadline = time.monotonic() + 60
            while True:
                try:
                    element = request(endpoint, "POST", prefix + "/element", {"using": "css selector", "value": "#gridcraft_loading"})
                    element_id = element["element-6066-11e4-a52e-4f735466cecf"]
                    if request(endpoint, "GET", prefix + f"/element/{element_id}/css/display") == "none":
                        break
                except urllib.error.HTTPError as error:
                    if error.code == 404:
                        break
                    raise
                if time.monotonic() >= deadline:
                    raise RuntimeError("Gridcraft loading indicator did not clear")
                time.sleep(0.1)
            for point in args.click:
                x, y = (int(value) for value in point.split(","))
                request(endpoint, "POST", prefix + "/actions", {"actions": [{"type": "pointer", "id": "mouse", "parameters": {"pointerType": "mouse"}, "actions": [
                    {"type": "pointerMove", "duration": 0, "x": x, "y": y, "origin": "viewport"},
                    {"type": "pointerDown", "button": 0}, {"type": "pointerUp", "button": 0}
                ]}]})
                time.sleep(0.2)
            time.sleep(0.5)
            png = base64.b64decode(request(endpoint, "GET", prefix + "/screenshot"), validate=True)
            args.output.parent.mkdir(parents=True, exist_ok=True)
            args.output.write_bytes(png)
            record["screenshot_sha256"] = hashlib.sha256(png).hexdigest()
            record["browser_log"] = request(endpoint, "POST", prefix + "/log", {"type": "browser"})
            record["passed"] = True
            args.output.with_suffix(".json").write_text(json.dumps(record, indent=2) + "\n", encoding="utf-8")
            print(json.dumps({"output": str(args.output), "sha256": record["screenshot_sha256"], "passed": True}))
        finally:
            if session is not None:
                try:
                    request(endpoint, "DELETE", f"/session/{session}")
                except OSError:
                    pass
            driver.terminate()
            try:
                driver.wait(timeout=5)
            except subprocess.TimeoutExpired:
                driver.kill()
                driver.wait(timeout=5)
            server.shutdown()
            server.server_close()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
