"""Build actual worker-browser tests and record Cargo's exact emitted WASM path."""
from __future__ import annotations
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
from build_trunk import inputs


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--cargo", default="cargo")
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args()
    sdk = Path(__file__).resolve().parents[2]
    def source_inputs() -> dict[str, str]:
        sources = inputs(sdk.parent)
        sources.update({path.relative_to(sdk.parent).as_posix(): hashlib.sha256(path.read_bytes()).hexdigest()
                        for path in (sdk / "tests/worker-browser").rglob("*")
                        if path.is_file() and path.suffix in (".rs", ".toml", ".lock")})
        return sources
    before = source_inputs()
    command = [args.cargo, "test", "--manifest-path", "tests/worker-browser/Cargo.toml", "--target", "wasm32-unknown-unknown",
               "--test", "worker", "--no-run", "--locked", "--message-format=json-render-diagnostics"]
    process = subprocess.run(command, cwd=sdk, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                             text=True, encoding="utf-8", errors="replace", timeout=900)
    artifacts = []
    diagnostics = []
    for line in process.stdout.splitlines():
        try:
            item = json.loads(line)
        except ValueError:
            continue
        if item.get("reason") == "compiler-artifact" and item.get("target", {}).get("name") == "worker":
            artifacts.extend(Path(name) for name in item["filenames"] if name.endswith(".wasm"))
        if item.get("reason") == "compiler-message" and item.get("message", {}).get("level") == "error":
            diagnostics.append(item["message"].get("rendered", ""))
    if process.returncode or len(artifacts) != 1:
        print(process.stderr[-10000:])
        print("\n".join(diagnostics))
        raise RuntimeError("Worker browser test build failed or did not emit exactly one test WASM artifact")
    artifact = artifacts[0]
    record = {"command": command, "exit_code": process.returncode, "artifact": str(artifact),
              "artifact_sha256": hashlib.sha256(artifact.read_bytes()).hexdigest(),
              "input_files": before,
              "source_digest_sha256": hashlib.sha256(json.dumps(before, sort_keys=True).encode()).hexdigest(),
              "source_changed_during_build": before != source_inputs()}
    record["accepted"] = not record["source_changed_during_build"]
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(record, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({key: value for key, value in record.items() if key != "input_files"}))
    return 0 if record["accepted"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
