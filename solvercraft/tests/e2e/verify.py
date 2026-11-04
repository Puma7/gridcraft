"""Reproduce SDK and full Gridcraft gates without modifying manifests or workflows.

Run with Rust 1.95 on PATH and an isolated CARGO_TARGET_DIR/CARGO_HOME as needed.
Each invocation records exact commands, source hashes, tool versions and full logs.
No skipped gate is represented as passing. The --list action has no side effects.
"""
from __future__ import annotations

import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import subprocess
import sys
import time

SDK = Path(__file__).resolve().parents[2]
HOST = SDK.parent
GATES = {
    "sdk-fmt": (SDK, ["fmt", "--all", "--", "--check"]),
    "sdk-clippy": (SDK, ["clippy", "--workspace", "--all-targets", "--locked", "--", "-D", "warnings"]),
    "sdk-tests": (SDK, ["test", "--workspace", "--locked"]),
    "cli-advanced": (SDK, ["test", "-p", "solvercraft-cli", "--test", "advanced_process", "--locked"]),
    "host-engine": (HOST, ["test", "-p", "gridcraft-engine", "--locked"]),
    "host-fmt": (HOST, ["fmt", "--all", "--", "--check"]),
    "host-clippy": (HOST, ["clippy", "--workspace", "--all-targets", "--keep-going", "--locked", "--", "-D", "warnings"]),
    "host-tests": (HOST, ["test", "--workspace", "--lib", "--bins", "--tests", "--locked"]),
    "host-doctests": (HOST, ["test", "--workspace", "--doc", "--locked"]),
    "host-ui": (HOST, ["test", "-p", "gridcraft-ui-egui", "--test", "solvercraft_acceptance", "--locked"]),
    "host-charts": (HOST, ["test", "-p", "gridcraft-ui-egui", "--lib", "solvercraft::tests", "--locked"]),
    "host-ci": (HOST, ["xtask", "ci"]),
    "host-wasm": (HOST, ["xtask", "wasm"]),
    "host-assets": (HOST, ["xtask", "assets"]),
    "host-layers": (HOST, ["xtask", "layers"]),
    "host-parity": (HOST, ["xtask", "parity", "--check"]),
    "host-keytips": (HOST, ["xtask", "keytips", "--check"]),
    "wasm-fmt": (SDK, ["fmt", "--manifest-path", "tests/wasm-smoke/Cargo.toml", "--all", "--", "--check"]),
}


def identity(cargo: str) -> dict[str, object]:
    versions = {}
    commands = (("cargo", [cargo, "--version"]), ("rustc", [os.environ.get("RUSTC", "rustc"), "-Vv"]),
                ("cargo_fmt", [cargo, "fmt", "--version"]), ("rustfmt", [os.environ.get("RUSTFMT", "rustfmt"), "--version"]),
                ("clippy", [cargo, "clippy", "--version"]))
    for name, command in commands:
        completed = subprocess.run(command, capture_output=True, text=True, timeout=15)
        versions[name] = {"exit_code": completed.returncode, "output": completed.stdout.strip()}
    sources = {}
    canonical_sources = {}
    compilation_inputs = set()
    def add_source(path: Path) -> None:
        if not path.is_file() or not path.resolve().is_relative_to(HOST.resolve()):
            return
        relative = path.resolve().relative_to(HOST.resolve()).as_posix()
        data = path.read_bytes()
        sources[relative] = hashlib.sha256(data).hexdigest()
        try:
            data.decode("utf-8")
            canonical = data.replace(b"\r\n", b"\n")
        except UnicodeDecodeError:
            canonical = data
        canonical_sources[relative] = hashlib.sha256(canonical).hexdigest()
    for base in (SDK / "crates", HOST / "crates", HOST / "apps", HOST / "xtask"):
        for path in sorted(base.rglob("*")):
            if path.is_file() and not path.is_symlink() and path.suffix in (".rs", ".toml", ".lock", ".json", ".txt", ".csv", ".html"):
                add_source(path)
                if path.suffix == ".rs":
                    for match in re.finditer(r'include_(?:str|bytes)!\s*\(\s*"([^"\r\n]+)"\s*\)', path.read_text(encoding="utf-8")):
                        included = (path.parent / match.group(1)).resolve()
                        if included.is_file() and included.is_relative_to(HOST.resolve()):
                            add_source(included)
                            compilation_inputs.add(included.relative_to(HOST.resolve()).as_posix())
    for path in (SDK / "Cargo.toml", SDK / "Cargo.lock", HOST / "Cargo.toml", HOST / "Cargo.lock"):
        if path.is_file():
            add_source(path)
    digest = hashlib.sha256(json.dumps(sources, sort_keys=True).encode()).hexdigest()
    canonical_digest = hashlib.sha256(json.dumps(canonical_sources, sort_keys=True).encode()).hexdigest()
    executables = {name: shutil.which(name) for name in ("cargo", "rustc", "cargo-fmt", "rustfmt", "cargo-clippy", "clippy-driver")}
    build_environment = {name: os.environ.get(name) for name in
                         ("CARGO_TARGET_DIR", "CARGO_HOME", "RUSTUP_HOME", "CARGO_BUILD_JOBS", "CARGO_NET_GIT_FETCH_WITH_CLI", "TMP", "TEMP",
                          "CRAFT_FONTS_DIR", "CRAFT_FONTS_REQUIRED", "GRIDCRAFT_NO_PREFS")}
    font_input = None
    if os.environ.get("CRAFT_FONTS_DIR"):
        font_record = Path(os.environ["CRAFT_FONTS_DIR"]) / "solvercraft-font-input-record.json"
        if font_record.is_file():
            font_input = {"path": str(font_record), "sha256": hashlib.sha256(font_record.read_bytes()).hexdigest(),
                          "record": json.loads(font_record.read_text(encoding="utf-8"))}
    return {"versions": versions, "resolved_executables": executables, "build_environment": build_environment, "font_input": font_input,
            "rustfmt_override": os.environ.get("RUSTFMT"), "platform": platform.platform(), "architecture": platform.machine(),
            "logical_cpus": os.cpu_count(), "source_digest_sha256": digest, "source_files": sources,
            "canonical_source_files": canonical_sources, "canonical_source_digest_sha256": canonical_digest,
            "canonicalization": "CRLF-to-LF for UTF-8 source/compilation data; non-UTF8 included bytes remain unchanged. Raw hashes govern stability checks.",
            "compilation_input_files": sorted(compilation_inputs)}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--gate", action="append", choices=GATES)
    parser.add_argument("--list", action="store_true")
    parser.add_argument("--cargo", default="cargo")
    parser.add_argument("--report-dir", type=Path, default=SDK / "docs/implementation/gates")
    parser.add_argument("--timeout", type=int, default=7200)
    args = parser.parse_args()
    if args.list:
        print(json.dumps({name: {"cwd": str(cwd), "command": [args.cargo, *command]}
                          for name, (cwd, command) in GATES.items()}, indent=2))
        return 0
    if not args.gate:
        parser.error("Select at least one --gate or use --list")
    if not 1 <= args.timeout <= 28800:
        parser.error("Timeout must be between 1 and 28800 seconds")
    args.report_dir.mkdir(parents=True, exist_ok=True)
    report = {"schema": "solvercraft.verification.v1", "started_utc": datetime.now(timezone.utc).isoformat(),
              "identity": identity(args.cargo), "gates": []}
    failed = False
    previous_identity = report["identity"]
    for gate in args.gate:
        cwd, arguments = GATES[gate]
        command = [args.cargo, *arguments]
        log = args.report_dir / f"{gate}.log"
        print(f"Running {gate}: {' '.join(command)}", flush=True)
        start = time.perf_counter()
        timed_out = False
        with log.open("w", encoding="utf-8") as output:
            process = subprocess.Popen(command, cwd=cwd, stdout=output, stderr=subprocess.STDOUT)
            try:
                code = process.wait(timeout=args.timeout)
            except subprocess.TimeoutExpired:
                timed_out = True
                # This PID is the child just created here. Stop only its subtree.
                if os.name == "nt":
                    subprocess.run(["taskkill", "/PID", str(process.pid), "/T", "/F"], capture_output=True, timeout=15)
                else:
                    process.kill()
                code = process.wait(timeout=15)
        after_identity = identity(args.cargo)
        changed = previous_identity["source_digest_sha256"] != after_identity["source_digest_sha256"]
        item = {"name": gate, "command": command, "cwd": str(cwd), "log": str(log),
                "exit_code": code, "timed_out": timed_out, "elapsed_seconds": time.perf_counter() - start}
        item.update({"source_digest_before": previous_identity["source_digest_sha256"],
                     "source_digest_after": after_identity["source_digest_sha256"], "source_changed_during_run": changed,
                     "accepted": code == 0 and not timed_out and not changed})
        report["gates"].append(item)
        report["identity_after"] = after_identity
        previous_identity = after_identity
        failed |= code != 0 or timed_out or changed
        report["finished_utc"] = datetime.now(timezone.utc).isoformat()
        (args.report_dir / "report.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
        print(json.dumps(item), flush=True)
    return int(failed)


if __name__ == "__main__":
    sys.exit(main())
