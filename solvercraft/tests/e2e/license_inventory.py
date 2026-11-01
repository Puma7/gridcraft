"""Record the locked SDK dependency licenses and resolved feature graph."""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import subprocess


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--cargo", default="cargo")
    parser.add_argument("--output", type=Path)
    parser.add_argument("--manifest-path", type=Path)
    args = parser.parse_args()
    sdk = Path(__file__).resolve().parents[2]
    workspace = args.manifest_path.resolve().parent if args.manifest_path else sdk
    if not (workspace / "Cargo.toml").is_file():
        parser.error("Manifest must identify an existing Cargo.toml")
    output = args.output or sdk / "docs/implementation/dependency-inventory.json"
    metadata = subprocess.run([args.cargo, "metadata", "--locked", "--format-version", "1"],
                              cwd=workspace, capture_output=True, text=True, timeout=120, check=True)
    data = json.loads(metadata.stdout)
    resolved = {node["id"]: node for node in data["resolve"]["nodes"]}
    packages = []
    for package in sorted(data["packages"], key=lambda item: (item["name"], item["version"], item["id"])):
        if package["id"] not in resolved:
            continue
        manifest = Path(package["manifest_path"])
        evidence = []
        for candidate in sorted(manifest.parent.iterdir()):
            if candidate.is_file() and candidate.name.upper().startswith(("LICENSE", "LICENCE", "COPYING", "NOTICE")):
                evidence.append({"path": str(candidate), "sha256": hashlib.sha256(candidate.read_bytes()).hexdigest(),
                                 "bytes": candidate.stat().st_size})
        packages.append({key: package.get(key) for key in
                         ("name", "version", "license", "license_file", "source", "repository")}
                        | {"id": package["id"], "manifest_path": str(manifest),
                           "manifest_sha256": hashlib.sha256(manifest.read_bytes()).hexdigest(),
                           "license_notice_evidence": evidence,
                           "enabled_features": resolved[package["id"]]["features"]})
    tree = subprocess.run([args.cargo, "tree", "--locked", "--workspace", "-e", "features"],
                          cwd=workspace, capture_output=True, text=True, timeout=120, check=True)
    output.parent.mkdir(parents=True, exist_ok=True)
    feature_path = output.with_suffix(".features.txt")
    feature_path.write_text(tree.stdout, encoding="utf-8")
    bundled = []
    direction_data = workspace / "crates/math/src/sobol_directions.rs"
    direction_license = workspace / "crates/math/third_party/BOOST_LICENSE_1_0.txt"
    if direction_data.is_file() and direction_license.is_file():
        bundled.append({"name": "Sobol initial direction data, dimensions 2..64", "upstream_version": "Boost 1.88.0",
                        "source": "https://github.com/boostorg/random/blob/22338471aa0f548bc92ddc351600ec4d510f164f/include/boost/random/detail/sobol_table.hpp",
                        "copyright": "Justinas Vygintas Daugmaudis 2010-2018; data derived upstream from Joe-Kuo (2008)",
                        "license": "BSL-1.0", "path": str(direction_data), "data_sha256": hashlib.sha256(direction_data.read_bytes()).hexdigest(),
                        "license_path": str(direction_license), "license_sha256": hashlib.sha256(direction_license.read_bytes()).hexdigest(),
                        "scope": "First 63 data rows only; no upstream algorithm code"})
    inventory = {"schema": "solvercraft.dependency-inventory.v1",
                 "lock_sha256": hashlib.sha256((workspace / "Cargo.lock").read_bytes()).hexdigest(),
                 "packages": packages, "bundled_data": bundled, "feature_tree": feature_path.name,
                 "note": "SPDX metadata inventory; does not replace the upstream license texts or legal review."}
    output.write_text(json.dumps(inventory, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"packages": len(packages), "output": str(output), "features": str(feature_path)}))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
