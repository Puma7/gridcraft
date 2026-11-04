"""Materialize only permitted fonts from the exact existing Gridcraft CI pin.

No font bytes enter Gridcraft. Each font is SHA-256 checked against the pinned
upstream manifest; barred families are not downloaded. Licenses are retained.
"""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path, PurePosixPath
import urllib.parse
import urllib.request

REVISION = "b076417b5880bd4154bd22f00695b1d28c6bea83"
BASE = f"https://raw.githubusercontent.com/storytold/craft-fonts/{REVISION}/"
BARRED = ("Adobe", "Source Han", "Source Sans", "Source Serif", "Noto Sans CJK", "Noto Serif CJK")


def relative(value: str) -> str:
    path = PurePosixPath(value)
    if path.is_absolute() or ".." in path.parts or "\\" in value:
        raise ValueError("Unsafe upstream manifest path")
    return path.as_posix()


def fetch(name: str, limit: int) -> bytes:
    url = BASE + urllib.parse.quote(relative(name), safe="/")
    with urllib.request.urlopen(url, timeout=60) as response:
        data = response.read(limit + 1)
    if len(data) > limit:
        raise ValueError(f"Font input exceeds budget: {name}")
    return data


def write(root: Path, name: str, data: bytes) -> None:
    target = (root / relative(name)).resolve()
    if not target.is_relative_to(root):
        raise ValueError("Destination escapes the font cache")
    if target.exists() and target.read_bytes() != data:
        raise ValueError(f"Refusing to replace differing existing input: {name}")
    target.parent.mkdir(parents=True, exist_ok=True)
    if not target.exists():
        with target.open("xb") as output:
            output.write(data)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("destination", type=Path)
    args = parser.parse_args()
    root = args.destination.resolve()
    root.mkdir(parents=True, exist_ok=True)
    records = []
    licenses = set()
    total = 0
    for manifest_name in ("fonts/manifest.txt", "fonts/simplified-chinese-manifest.txt"):
        manifest = fetch(manifest_name, 256 * 1024)
        write(root, manifest_name, manifest)
        for line in manifest.decode().splitlines():
            if not line.strip() or line.lstrip().startswith("#"):
                continue
            fields = [field.strip() for field in line.split(" | ")]
            if len(fields) != 8:
                raise ValueError("Unexpected craft-fonts manifest schema")
            family, style, name, scripts, license_name, license_path, expected, source = fields
            if family.startswith(BARRED):
                continue
            name = relative(name)
            data = fetch(name, 64 * 1024 * 1024)
            if hashlib.sha256(data).hexdigest().lower() != expected.lower():
                raise ValueError(f"Pinned font SHA-256 mismatch: {name}")
            total += len(data)
            if total > 256 * 1024 * 1024:
                raise ValueError("Permitted font aggregate exceeds budget")
            write(root, name, data)
            licenses.add(relative(license_path))
            records.append({"family": family, "style": style, "path": name, "scripts": scripts,
                            "sha256": expected, "bytes": len(data), "license": license_name,
                            "license_file": license_path, "source": source})
    license_records = []
    for name in sorted(licenses):
        data = fetch(name, 1024 * 1024)
        write(root, name, data)
        license_records.append({"path": name, "sha256": hashlib.sha256(data).hexdigest()})
    record = {"schema": "solvercraft.craft-font-input.v1", "revision": REVISION,
              "source": f"https://github.com/storytold/craft-fonts/tree/{REVISION}",
              "fonts": records, "licenses": license_records, "bytes": total,
              "barred_families_downloaded": False, "gridcraft_source_modified": False}
    (root / "solvercraft-font-input-record.json").write_text(json.dumps(record, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"fonts": len(records), "bytes": total, "revision": REVISION, "destination": str(root)}))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
