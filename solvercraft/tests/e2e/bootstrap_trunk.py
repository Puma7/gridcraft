"""Install the pinned official Trunk portable tool; verify its published SHA-256."""
from __future__ import annotations

import argparse
import hashlib
import io
import json
from pathlib import Path
import re
import subprocess
import urllib.request
import zipfile

VERSION = "0.21.14"
URL = f"https://github.com/trunk-rs/trunk/releases/download/v{VERSION}/trunk-x86_64-pc-windows-msvc.zip"


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("destination", type=Path)
    args = parser.parse_args()
    target = args.destination.resolve() / f"trunk-{VERSION}"
    target.mkdir(parents=True, exist_ok=True)
    with urllib.request.urlopen(URL + ".sha256", timeout=30) as response:
        expected = response.read(4096).decode().split()[0]
    if not re.fullmatch("[0-9a-fA-F]{64}", expected):
        raise ValueError("Unexpected official Trunk checksum format")
    with urllib.request.urlopen(URL, timeout=30) as response:
        data = response.read(32 * 1024 * 1024 + 1)
    if len(data) > 32 * 1024 * 1024 or hashlib.sha256(data).hexdigest().lower() != expected.lower():
        raise ValueError("Trunk download size/hash verification failed")
    with zipfile.ZipFile(io.BytesIO(data)) as archive:
        if sum(item.file_size for item in archive.infolist()) > 128 * 1024 * 1024:
            raise ValueError("Trunk extraction exceeds budget")
        for item in archive.infolist():
            if item.is_dir():
                continue
            path = (target / item.filename).resolve()
            if not path.is_relative_to(target):
                raise ValueError("Unsafe Trunk archive path")
            content = archive.read(item)
            if path.exists() and path.read_bytes() != content:
                raise ValueError("Refusing to replace differing existing Trunk files")
            path.parent.mkdir(parents=True, exist_ok=True)
            if not path.exists():
                with path.open("xb") as output:
                    output.write(content)
    executable = target / "trunk.exe"
    result = subprocess.run([str(executable), "--version"], capture_output=True, text=True, check=True, timeout=15)
    record = {"tool": "Trunk", "version": result.stdout.strip(), "source": URL,
              "checksum_source": URL + ".sha256", "archive_sha256": expected,
              "executable": str(executable), "executable_sha256": hashlib.sha256(executable.read_bytes()).hexdigest(),
              "license": "MIT OR Apache-2.0; upstream https://github.com/trunk-rs/trunk"}
    (target / "tool-record.json").write_text(json.dumps(record, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(record))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
