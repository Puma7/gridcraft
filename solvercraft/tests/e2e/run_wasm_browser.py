"""Run the isolated Rust WASM tests in a real, disposable headless Chrome session.

Uses wasm-bindgen-test-runner's documented remote WebDriver endpoint. All
processes started by this script are bounded and only its own driver is stopped.
The driver itself creates a temporary browser profile; user profiles are unused.
"""
from __future__ import annotations

import argparse
from contextlib import nullcontext
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import socket
import subprocess
import sys
import tempfile
import time
import urllib.request


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--driver", type=Path, required=True)
    parser.add_argument("--runner", type=Path, required=True)
    parser.add_argument("--browser", type=Path, required=True)
    parser.add_argument("--cargo", type=Path, default=Path("cargo"))
    parser.add_argument("--report", type=Path)
    parser.add_argument("--work-dir", type=Path)
    parser.add_argument("--test-wasm", type=Path)
    parser.add_argument("--assets-dir", type=Path)
    parser.add_argument("--production-build-report", type=Path,
                        help="Require assets to match an accepted unchanged-source Trunk build")
    parser.add_argument("--app-screenshot", type=Path)
    parser.add_argument("--test-filter")
    parser.add_argument("--app-click", action="append", default=[], help="CSS viewport x,y pointer action after app readiness")
    args = parser.parse_args()
    sdk = Path(__file__).resolve().parents[2]
    for name in ("driver", "runner", "browser"):
        if not getattr(args, name).is_file():
            parser.error(f"{name} file does not exist")
    if args.test_wasm and (not args.test_wasm.is_file() or not args.assets_dir or not args.assets_dir.is_dir()):
        parser.error("Direct test WASM execution requires an existing --test-wasm and --assets-dir")
    if args.production_build_report and not args.test_wasm:
        parser.error("--production-build-report requires direct test WASM and assets")
    with socket.socket() as listener:
        listener.bind(("127.0.0.1", 0))
        port = listener.getsockname()[1]
    endpoint = f"http://127.0.0.1:{port}"
    start = time.perf_counter()
    result: dict[str, object] = {
        "schema": "solvercraft.browser-smoke.v1",
        "driver_sha256": hashlib.sha256(args.driver.read_bytes()).hexdigest(),
        "runner_sha256": hashlib.sha256(args.runner.read_bytes()).hexdigest(),
        "browser_sha256": hashlib.sha256(args.browser.read_bytes()).hexdigest(),
        "browser": str(args.browser),
        "endpoint": endpoint,
        "actual_browser": True,
        "platform": platform.platform(),
    }
    source_paths = [path for path in (sdk / "crates").rglob("*")
                    if path.is_file() and path.suffix in (".rs", ".toml")]
    source_paths.extend([sdk / "Cargo.lock", sdk / "tests/wasm-smoke/Cargo.lock",
                         sdk / "tests/wasm-smoke/tests/browser.rs"])
    source_hashes = {path.relative_to(sdk).as_posix(): hashlib.sha256(path.read_bytes()).hexdigest()
                     for path in sorted(source_paths)}
    result["source_files"] = source_hashes
    result["source_digest_sha256"] = hashlib.sha256(json.dumps(source_hashes, sort_keys=True).encode()).hexdigest()
    rustc = subprocess.run([os.environ.get("RUSTC", "rustc"), "-Vv"], capture_output=True, text=True, timeout=15)
    result["rustc"] = rustc.stdout.strip()
    if args.test_wasm:
        result["test_wasm_sha256"] = hashlib.sha256(args.test_wasm.read_bytes()).hexdigest()
        result["production_assets"] = {path.relative_to(args.assets_dir).as_posix(): hashlib.sha256(path.read_bytes()).hexdigest()
                                       for path in sorted(args.assets_dir.rglob("*")) if path.is_file()}
        if args.production_build_report:
            build = json.loads(args.production_build_report.read_text(encoding="utf-8"))
            expected = {name: item["sha256"] for name, item in build.get("generated_assets", {}).items()}
            if not build.get("accepted") or expected != result["production_assets"]:
                raise ValueError("Production assets do not match an accepted unchanged-source Trunk build")
            result["production_build"] = {
                "report": str(args.production_build_report),
                "report_sha256": hashlib.sha256(args.production_build_report.read_bytes()).hexdigest(),
                "source_digest_sha256": build["source_digest_sha256"],
                "command": build["command"], "cwd": build["cwd"],
            }
    work_dir = args.work_dir or sdk.parents[1] / "solvercraft-browser-runs"
    work_dir.mkdir(parents=True, exist_ok=True)
    # Keep the tiny per-run logs/capabilities for review. Do not use a sandbox's
    # redirected OS temp folder, which can have different write permissions.
    with nullcontext(tempfile.mkdtemp(prefix="solvercraft-wasm-", dir=work_dir)) as temporary:
        temp = Path(temporary)
        capabilities = temp / "webdriver.json"
        capabilities.write_text(json.dumps({"goog:chromeOptions": {
            "binary": str(args.browser), "args": ["window-size=1600,1100"]
        }}), encoding="utf-8")
        with (temp / "driver.log").open("w+", encoding="utf-8") as driver_log:
            driver = subprocess.Popen([str(args.driver), f"--port={port}"],
                                      stdout=driver_log, stderr=subprocess.STDOUT)
            try:
                deadline = time.monotonic() + 20
                while True:
                    try:
                        with urllib.request.urlopen(endpoint + "/status", timeout=1) as response:
                            status = json.load(response)
                        if status["value"]["ready"]:
                            result["webdriver"] = status["value"]
                            break
                    except (OSError, ValueError, KeyError):
                        pass
                    if driver.poll() is not None or time.monotonic() >= deadline:
                        raise RuntimeError("Isolated ChromeDriver did not become ready")
                    time.sleep(0.1)
                environment = os.environ.copy()
                environment.update({
                    "CHROMEDRIVER_REMOTE": endpoint,
                    "CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER": str(args.runner.resolve()),
                    "WASM_BINDGEN_TEST_WEBDRIVER_JSON": str(capabilities),
                    "WASM_BINDGEN_USE_BROWSER": "1",
                    "WASM_BINDGEN_TEST_TIMEOUT": "60",
                })
                environment.pop("NO_HEADLESS", None)
                if args.test_wasm:
                    command = [str(args.runner.resolve()), str(args.test_wasm.resolve()), "--nocapture"]
                    if args.test_filter:
                        command.append(args.test_filter)
                    working_directory = args.assets_dir.resolve()
                else:
                    command = [str(args.cargo), "test", "--manifest-path",
                               str(sdk / "tests/wasm-smoke/Cargo.toml"), "--target",
                               "wasm32-unknown-unknown", "--locked", "--test", "browser",
                               "--", "--nocapture"]
                    working_directory = sdk
                result["command"] = command
                result["cwd"] = str(working_directory)
                with (temp / "test-output.log").open("w+", encoding="utf-8") as test_log:
                    process = subprocess.Popen(command, cwd=working_directory, env=environment, stdout=test_log, stderr=subprocess.STDOUT)
                    deadline = time.monotonic() + 600
                    captured = False
                    interacted = False
                    while process.poll() is None:
                        if time.monotonic() >= deadline:
                            process.kill()
                            process.wait(timeout=5)
                            raise RuntimeError("Browser test runner exceeded 600 seconds")
                        if args.app_screenshot and not captured:
                            try:
                                with urllib.request.urlopen(endpoint + "/sessions", timeout=1) as response:
                                    sessions = json.load(response)["value"]
                                if sessions:
                                    session_id = sessions[0]["id"]
                                    query = urllib.request.Request(endpoint + f"/session/{session_id}/element",
                                        data=json.dumps({"using":"css selector","value":"iframe[data-solvercraft-app-ready='true']"}).encode(),
                                        headers={"Content-Type":"application/json"}, method="POST")
                                    with urllib.request.urlopen(query, timeout=1) as response:
                                        found = json.load(response)
                                    element = found.get("value", {})
                                    # The official runner starts a legacy JSON Wire session.
                                    # That driver can report "no such element" as HTTP 200,
                                    # with nonzero status, rather than an HTTP error.
                                    if found.get("status", 0) not in (0, None) or not isinstance(element, dict) or not any(
                                            key in element for key in ("ELEMENT", "element-6066-11e4-a52e-4f735466cecf")):
                                        raise ValueError("Actual app is not ready for a screenshot")
                                    if not interacted and args.app_click:
                                        for point in args.app_click:
                                            x, y = (int(value) for value in point.split(","))
                                            action = urllib.request.Request(endpoint + f"/session/{session_id}/actions",
                                                data=json.dumps({"actions":[{"type":"pointer","id":"app-pointer","parameters":{"pointerType":"mouse"},"actions":[
                                                    {"type":"pointerMove","duration":0,"x":x,"y":y,"origin":"viewport"},
                                                    {"type":"pointerDown","button":0},{"type":"pointerUp","button":0}]}]}).encode(),
                                                headers={"Content-Type":"application/json"}, method="POST")
                                            with urllib.request.urlopen(action, timeout=1) as response:
                                                applied = json.load(response)
                                            if applied.get("status", 0) not in (0, None):
                                                raise ValueError("Actual app pointer action failed")
                                            time.sleep(0.2)
                                        result["app_pointer_actions"] = args.app_click
                                        interacted = True
                                    with urllib.request.urlopen(endpoint + f"/session/{session_id}/screenshot", timeout=3) as response:
                                        encoded = json.load(response)["value"]
                                    import base64
                                    png = base64.b64decode(encoded, validate=True)
                                    from io import BytesIO
                                    from PIL import Image
                                    colors = Image.open(BytesIO(png)).convert("RGB").getcolors(maxcolors=256)
                                    if colors is not None and len(colors) <= 8:
                                        # App initialization can precede its first paint. Keep
                                        # waiting rather than accept a blank canvas as visual proof.
                                        time.sleep(0.1)
                                        continue
                                    args.app_screenshot.parent.mkdir(parents=True, exist_ok=True)
                                    args.app_screenshot.write_bytes(png)
                                    result["app_screenshot"] = {"path": str(args.app_screenshot), "sha256": hashlib.sha256(png).hexdigest()}
                                    captured = True
                            except (OSError, ValueError, KeyError):
                                pass
                        time.sleep(0.1)
                    test_log.seek(0)
                    output = test_log.read(1024 * 1024)
                    print(output, end="", flush=True)
                    result["runner_exit_code"] = process.returncode
                    result["exit_code"] = process.returncode if not args.app_screenshot or captured else 1
                    result["output"] = output
                    if not args.test_wasm:
                        artifacts = [Path(name) for name in re.findall(r"\(([^()\r\n]+\.wasm)\)", output)]
                        if len(artifacts) == 1 and artifacts[0].is_file():
                            result["test_wasm_path"] = str(artifacts[0])
                            result["test_wasm_sha256"] = hashlib.sha256(artifacts[0].read_bytes()).hexdigest()
                        else:
                            result["artifact_identity_error"] = "Cargo did not identify exactly one existing test WASM artifact"
                            result["exit_code"] = 1
            finally:
                driver.terminate()
                try:
                    driver.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    driver.kill()
                    driver.wait(timeout=5)
                driver_log.seek(0)
                result["driver_output"] = driver_log.read(128 * 1024)
                result["elapsed_seconds"] = time.perf_counter() - start
                if args.report:
                    args.report.parent.mkdir(parents=True, exist_ok=True)
                    args.report.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    return int(result.get("exit_code", 1))


if __name__ == "__main__":
    sys.exit(main())
