"""Render the real Gridcraft Solvercraft pane offscreen with a synthetic workbook."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--cargo", default="cargo")
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--language", choices=("en", "de"), default="en")
    args = parser.parse_args()
    sdk = Path(__file__).resolve().parents[2]
    output = args.output.resolve()
    output.parent.mkdir(parents=True, exist_ok=True)
    environment = os.environ.copy()
    environment["GRIDCRAFT_NO_PREFS"] = "1"
    command = [args.cargo, "run", "--locked", "-p", "gridcraft-ui-egui", "--example", "snapshot", "--",
               "--size", "1600x1000", "--scale", "1", "--language", args.language,
               "--tab", "Data", "--pane", "solvercraft"]
    for cell, value in (("A1", "2"), ("B1", "=2*A1")):
        command.extend(["--cmd", "cell.set=" + json.dumps({"cell": cell, "input": value}, separators=(",", ":"))])
    command.append(str(output))
    completed = subprocess.run(command, cwd=sdk.parent, env=environment, timeout=600)
    if completed.returncode == 0 and output.is_file():
        print(json.dumps({"command": command, "output": str(output),
                          "sha256": hashlib.sha256(output.read_bytes()).hexdigest(), "exit_code": 0}))
    return completed.returncode


if __name__ == "__main__":
    raise SystemExit(main())
