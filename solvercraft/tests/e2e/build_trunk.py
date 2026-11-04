"""Build the real Trunk app/worker and bind assets to unchanged input sources."""
from __future__ import annotations

import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import subprocess
import time


def inputs(host: Path) -> dict[str, str]:
    paths = []
    for directory in ("crates", "apps", "xtask", "solvercraft/crates", "assets", "contributors"):
        base = host / directory
        paths.extend(path for path in base.rglob("*") if path.is_file()
                     and path.suffix in (".rs", ".toml", ".lock", ".html", ".ico", ".png", ".ttf", ".json"))
    paths.extend(host / name for name in ("Cargo.toml", "Cargo.lock", "solvercraft/Cargo.toml", "solvercraft/Cargo.lock"))
    result = {path.relative_to(host).as_posix(): hashlib.sha256(path.read_bytes()).hexdigest()
              for path in sorted(set(paths)) if path.is_file()}
    if os.environ.get("CRAFT_FONTS_DIR"):
        fonts = Path(os.environ["CRAFT_FONTS_DIR"])
        result.update({"external-craft-fonts/" + path.relative_to(fonts).as_posix(): hashlib.sha256(path.read_bytes()).hexdigest()
                       for path in sorted(fonts.rglob("*")) if path.is_file()})
    return result


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--trunk", type=Path, required=True)
    parser.add_argument("--source-root", type=Path, default=Path(__file__).resolve().parents[3])
    parser.add_argument("--dist", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args()
    host, dist = args.source_root.resolve(), args.dist.resolve()
    if not args.trunk.is_file() or not (host / "apps/gridcraft-web/index.html").is_file():
        parser.error("Existing Trunk executable and full Gridcraft source required")
    if dist == host or host.is_relative_to(dist) or dist.is_relative_to(host):
        parser.error("Use a separate task-specific generated-asset directory")
    before = inputs(host)
    command = [str(args.trunk.resolve()), "build", "--dist", str(dist)]
    record = {"schema": "solvercraft.trunk-build.v1", "started_utc": datetime.now(timezone.utc).isoformat(),
              "command": command, "cwd": str(host / "apps/gridcraft-web"), "input_files": before,
              "source_digest_sha256": hashlib.sha256(json.dumps(before, sort_keys=True).encode()).hexdigest(),
              "trunk_sha256": hashlib.sha256(args.trunk.read_bytes()).hexdigest(),
              "environment": {name: os.environ.get(name) for name in
                  ("CARGO_TARGET_DIR", "CARGO_HOME", "RUSTUP_HOME", "RUSTC", "RUSTDOC", "CRAFT_FONTS_DIR", "CRAFT_FONTS_REQUIRED", "NO_COLOR")}}
    args.report.parent.mkdir(parents=True, exist_ok=True)
    log = args.report.with_suffix(".log")
    start = time.perf_counter()
    with log.open("w", encoding="utf-8") as stream:
        process = subprocess.run(command, cwd=host / "apps/gridcraft-web", stdout=stream,
                                 stderr=subprocess.STDOUT, timeout=1800)
    after = inputs(host)
    record.update({"exit_code": process.returncode, "elapsed_seconds": time.perf_counter() - start,
                   "source_changed_during_build": before != after, "log": str(log),
                   "accepted": process.returncode == 0 and before == after})
    if process.returncode == 0:
        record["generated_assets"] = {path.relative_to(dist).as_posix():
            {"sha256": hashlib.sha256(path.read_bytes()).hexdigest(), "bytes": path.stat().st_size}
            for path in sorted(dist.rglob("*")) if path.is_file()}
    args.report.write_text(json.dumps(record, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({key: value for key, value in record.items() if key not in ("input_files", "generated_assets")}))
    return 0 if record["accepted"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
