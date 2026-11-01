"""Overlay a hash-verified source snapshot into an existing pinned Git checkout.

Never writes the active source. Never deletes files, changes Git HEAD/index or
copies build caches, evidence, tool caches, private configuration or Git metadata.
Rejects an unstable source or extra source files in the destination.
"""
from __future__ import annotations

import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import subprocess

BASE = "95f7b46ff94fe833b2f3356db58dc6754916f43c"
SKIP = {".git", ".agents", ".codex", ".aws", ".cache", "__pycache__", "target", "evidence", "node_modules", "plan", "log", "dist"}


def paths(root: Path) -> dict[str, Path]:
    output = {}
    for directory, folders, names in os.walk(root, followlinks=False):
        folders[:] = sorted(name for name in folders if name not in SKIP
                            and not name.startswith(("target-", "gates-"))
                            and not (Path(directory) / name).is_symlink())
        for name in sorted(names):
            path = Path(directory) / name
            if path.is_symlink() or path.suffix in (".log", ".pyc") or name.startswith(".env"):
                continue
            output[path.relative_to(root).as_posix()] = path
    return output


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("source", type=Path)
    parser.add_argument("destination", type=Path)
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args()
    source, destination = args.source.resolve(), args.destination.resolve()
    if source == destination or source.is_relative_to(destination) or destination.is_relative_to(source):
        parser.error("Source and destination must be separate directories")
    if not (destination / ".git").is_dir():
        parser.error("Destination must be an existing independent Git checkout")
    head = subprocess.run(["git", "-C", str(destination), "rev-parse", "HEAD"], capture_output=True, text=True, check=True).stdout.strip()
    if head != BASE:
        parser.error("Destination must retain the pinned original base HEAD")
    initial = paths(source)
    before = {}
    changed = []
    total_bytes = 0
    for relative, path in initial.items():
        if path.stat().st_size > 16 * 1024 * 1024:
            raise ValueError(f"Source file exceeds 16 MiB snapshot budget: {relative}")
        data = path.read_bytes()
        total_bytes += len(data)
        if total_bytes > 256 * 1024 * 1024:
            raise ValueError("Snapshot exceeds 256 MiB aggregate budget")
        before[relative] = hashlib.sha256(data).hexdigest()
        target = (destination / relative).resolve()
        if not target.is_relative_to(destination) or target.is_symlink():
            raise ValueError(f"Unsafe target: {relative}")
        if not target.exists() or target.read_bytes() != data:
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(data)
            changed.append(relative)
    after_paths = paths(source)
    after = {relative: hashlib.sha256(path.read_bytes()).hexdigest() for relative, path in after_paths.items()}
    if before != after:
        raise RuntimeError("Source changed while copying; snapshot is not accepted. Retry after a coherent checkpoint.")
    actual_paths = paths(destination)
    actual = {relative: hashlib.sha256(path.read_bytes()).hexdigest() for relative, path in actual_paths.items()}
    # The proof itself is deliberately outside the copied snapshot, or this
    # comparison would introduce an extra destination file after verification.
    if actual != before:
        extra = sorted(actual.keys() - before.keys())
        missing = sorted(before.keys() - actual.keys())
        different = [name for name in before.keys() & actual.keys() if before[name] != actual[name]]
        raise RuntimeError(f"Destination differs: extra={extra[:20]}, missing={missing[:20]}, changed={different[:20]}")
    record = {"schema": "solvercraft.source-snapshot.v1", "date_utc": datetime.now(timezone.utc).isoformat(),
              "source": str(source), "destination": str(destination), "git_base": head,
              "files": before, "files_written": changed, "file_count": len(before), "total_bytes": total_bytes,
              "digest_sha256": hashlib.sha256(json.dumps(before, sort_keys=True).encode()).hexdigest(),
              "source_unchanged_while_copying": True, "destination_matches_source": True}
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(record, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({key: value for key, value in record.items() if key not in ("files", "files_written")}))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
