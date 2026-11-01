"""Audit the frozen per-function matrix; optionally execute one real test.

Inventory success is not implementation acceptance. Unsupported functions have
no invented executable test and --run refuses to call their planned IDs.
Use a configured Rust1.95 toolchain, as for the workspace's ordinary test gate.
"""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[2]


def load_and_audit() -> tuple[dict, dict[str, dict]]:
    matrix = json.loads((ROOT / "docs/implementation/reference-implementation.json").read_text(encoding="utf-8"))
    catalog = json.loads((ROOT / "docs/implementation/reference-catalog.json").read_text(encoding="utf-8"))
    rows = matrix["functions"]
    expected = {f["name"] for f in catalog["functions"]}
    if len(rows) != 272 or {r["name"] for r in rows} != expected:
        raise ValueError("Matrix must contain exactly one row for every frozen function.")
    if len({r["name"] for r in rows}) != len(rows):
        raise ValueError("Duplicate function row.")
    source_hash = catalog["sources"][0]["sha256"]
    for row in rows:
        if row["source"]["sha256"] != source_hash or not row["source"]["pages"]:
            raise ValueError("Missing or incorrect source identity: " + row["name"])
        if not row["planned_exact_signature_test"] or not row["platforms"] or not row["guarantee"]:
            raise ValueError("Missing test/platform/guarantee status: " + row["name"])
        for test in row["tests"]:
            path = ROOT / test["path"]
            content = path.read_bytes()
            if hashlib.sha256(content).hexdigest() != test["source_file_sha256"]:
                raise ValueError("Test evidence source changed; refresh matrix: " + test["path"])
            leaf = test["test"].split("::")[-1]
            if not re.search(r"\bfn\s+" + re.escape(leaf) + r"\s*\(", content.decode("utf-8")):
                raise ValueError("Named executable test is absent: " + test["test"])
            if not re.fullmatch(r"[A-Za-z_]\w*(?:::[A-Za-z_]\w*)*", test["test"]):
                raise ValueError("Invalid Rust test selector.")
            if test["command"][:4] != ["cargo", "test", "--offline", "--locked"]:
                raise ValueError("Only offline locked Cargo test commands are admitted.")
            if test["command"][-3:] != [test["test"], "--", "--exact"]:
                raise ValueError("Test command does not execute its exact selector.")
    return matrix, {r["name"]: r for r in rows}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--run", metavar="FUNCTION", help="Execute the first actual evidence test for one frozen function.")
    parser.add_argument("--list-gaps", action="store_true")
    parser.add_argument("--timeout", type=int, default=120)
    args = parser.parse_args()
    matrix, rows = load_and_audit()
    if args.list_gaps:
        for row in rows.values():
            if row["r046_in_scope"] and row["sdk_status"] not in ("core_subset_tested", "core_parameter_subset_tested"):
                print(row["name"] + ": " + row["remaining_gap"])
    if args.run:
        row = rows.get(args.run)
        if row is None or not row["tests"]:
            raise ValueError("No actual executable evidence is mapped for this function; a planned test is not a passing test.")
        test = row["tests"][0]
        result = subprocess.run(test["command"], cwd=ROOT / test["working_directory"], capture_output=True,
                                text=True, encoding="utf-8", errors="replace", timeout=args.timeout, check=False)
        print(result.stdout, end="")
        print(result.stderr, end="", file=sys.stderr)
        if result.returncode:
            return result.returncode
        if sum(map(int, re.findall(r"test result: ok\. (\d+) passed", result.stdout))) == 0:
            raise ValueError("Cargo matched no passing tests; this is not execution evidence.")
        print("Executed actual test for " + args.run + "; scope/guarantee remains exactly as recorded.")
    else:
        print(json.dumps({"inventory_only": True, "functions": len(rows), "r046_functions": matrix["scope"]["r046_distribution_and_time_series_functions"],
                          "status_counts": matrix["scope"]["r046_status_counts"]}, indent=2))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (ValueError, KeyError, OSError, subprocess.TimeoutExpired) as exc:
        print(str(exc), file=sys.stderr)
        raise SystemExit(2) from exc
