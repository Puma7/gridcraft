"""Extract a pinned official portable NASM build into the requested task cache."""
from __future__ import annotations

import argparse
import hashlib
import io
import json
from pathlib import Path
import subprocess
import urllib.request
import zipfile

VERSION = "3.02"
URL = f"https://www.nasm.us/pub/nasm/releasebuilds/{VERSION}/win64/nasm-{VERSION}-win64.zip"


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("destination", type=Path)
    args = parser.parse_args()
    destination = args.destination.resolve()
    destination.mkdir(parents=True, exist_ok=True)
    with urllib.request.urlopen(URL, timeout=30) as response:
        data = response.read(16 * 1024 * 1024 + 1)
    if len(data) > 16 * 1024 * 1024:
        raise ValueError("NASM archive exceeds download budget")
    with zipfile.ZipFile(io.BytesIO(data)) as archive:
        if sum(item.file_size for item in archive.infolist()) > 64 * 1024 * 1024:
            raise ValueError("NASM archive exceeds extraction budget")
        for item in archive.infolist():
            if item.is_dir():
                continue
            path = (destination / item.filename).resolve()
            if not path.is_relative_to(destination):
                raise ValueError("Unsafe NASM archive path")
            path.parent.mkdir(parents=True, exist_ok=True)
            content = archive.read(item)
            if path.exists() and path.read_bytes() != content:
                raise ValueError("Refusing to replace a differing existing tool file")
            if not path.exists():
                with path.open("xb") as output:
                    output.write(content)
    executable = destination / f"nasm-{VERSION}" / "nasm.exe"
    version = subprocess.run([str(executable), "-v"], capture_output=True, text=True, check=True, timeout=15).stdout.strip()
    record = {"tool": "NASM", "version": version, "source": URL,
              "archive_sha256": hashlib.sha256(data).hexdigest(),
              "executable": str(executable), "executable_sha256": hashlib.sha256(executable.read_bytes()).hexdigest(),
              "license": "BSD-2-Clause; upstream LICENSE retained beside the tool"}
    (destination / "nasm-tool-record.json").write_text(json.dumps(record, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(record))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
