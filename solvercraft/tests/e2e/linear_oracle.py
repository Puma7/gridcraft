"""Independent small-LP oracle against the real Solvercraft executable.

Requires NumPy. It enumerates active-constraint vertices, rather than invoking a
simplex implementation or sharing Solvercraft's model validation code.
Synthetic models are deterministic; no benchmark corpus or network is used.
"""
from __future__ import annotations

import argparse
from datetime import datetime, timezone
import hashlib
import itertools
import json
from pathlib import Path
import random
import platform
import subprocess
import sys

import numpy as np

SEED = 20261031


def generated_model(index: int, rng: random.Random) -> dict:
    n = 2 if index % 3 == 0 else 3
    variables = [
        {"name": f"x{i}", "lower": float(rng.randint(-3, -1)),
         "upper": float(rng.randint(1, 5)), "kind": "continuous"}
        for i in range(n)
    ]
    constraints = []
    for row in range(4):
        coefficients = [rng.randint(-4, 4) for _ in range(n)]
        if not any(coefficients):
            coefficients[0] = 1
        # Zero remains feasible. Equalities make some polytopes lower-dimensional.
        relation = "equal" if index % 7 == 0 and row == 0 else "less_equal"
        rhs = 0 if relation == "equal" else rng.randint(0, 8)
        scale = 10.0 ** rng.randint(-2, 2)
        constraints.append({
            "name": f"r{row}",
            "terms": [{"variable": i, "coefficient": a * scale}
                      for i, a in enumerate(coefficients) if a],
            "relation": relation, "rhs": rhs * scale,
        })
    costs = [rng.randint(-5, 5) for _ in range(n)]
    if not any(costs):
        costs[0] = 1
    return {
        "schema_version": 1, "name": f"numpy-vertex-{index}",
        "variables": variables,
        "objective": {
            "direction": "minimize" if index % 2 else "maximize",
            "offset": float(rng.randint(-3, 3)),
            "linear": [{"variable": i, "coefficient": a} for i, a in enumerate(costs) if a],
            "quadratic": [],
        },
        "constraints": constraints,
    }


def inequalities(model: dict) -> tuple[np.ndarray, np.ndarray]:
    n = len(model["variables"])
    rows, rhs = [], []
    for i, variable in enumerate(model["variables"]):
        row = np.zeros(n)
        row[i] = 1.0
        rows.extend([row.copy(), -row.copy()])
        rhs.extend([variable["upper"], -variable["lower"]])
    for constraint in model["constraints"]:
        row = np.zeros(n)
        for term in constraint["terms"]:
            row[term["variable"]] += term["coefficient"]
        rows.append(row)
        rhs.append(constraint["rhs"])
        if constraint["relation"] == "equal":
            rows.append(-row)
            rhs.append(-constraint["rhs"])
    return np.asarray(rows, dtype=float), np.asarray(rhs, dtype=float)


def oracle(model: dict) -> tuple[float, int]:
    matrix, limits = inequalities(model)
    n = len(model["variables"])
    costs = np.zeros(n)
    for term in model["objective"]["linear"]:
        costs[term["variable"]] += term["coefficient"]
    objectives = []
    for active in itertools.combinations(range(len(limits)), n):
        selected = list(active)
        try:
            values = np.linalg.solve(matrix[selected], limits[selected])
        except np.linalg.LinAlgError:
            continue
        # Scale the verification tolerance per constraint, not the objective.
        tolerance = 1e-8 * (1.0 + np.abs(limits))
        if np.all(np.isfinite(values)) and np.all(matrix @ values <= limits + tolerance):
            objectives.append(float(costs @ values + model["objective"]["offset"]))
    if not objectives:
        raise ValueError("No feasible vertices found in a bounded, origin-feasible model")
    optimum = min(objectives) if model["objective"]["direction"] == "minimize" else max(objectives)
    return optimum, len(objectives)


def check_process(executable: Path, model: dict, optimum: float) -> dict:
    request = {"model": model, "options": {"seed": str(SEED)}}
    completed = subprocess.run(
        [str(executable), "solve", "-"], input=json.dumps(request),
        text=True, capture_output=True, timeout=20, check=False,
    )
    if completed.returncode != 0:
        raise ValueError(f"CLI exited {completed.returncode}: {completed.stderr}")
    if completed.stderr:
        raise ValueError(f"Successful process emitted stderr: {completed.stderr}")
    payload = json.loads(completed.stdout)
    result = payload["result"]
    if result["termination"] != "completed" or result["feasibility"] != "feasible" or result["optimality"] != "optimal":
        raise ValueError(f"Unexpected status: {result}")
    objective = result["objective"]
    if not np.isfinite(objective) or abs(objective - optimum) > 1e-6 * max(1.0, abs(optimum)):
        raise ValueError(f"Objective {objective}, independent optimum {optimum}")
    values = np.asarray(result["values"], dtype=float)
    matrix, limits = inequalities(model)
    if len(values) != len(model["variables"]) or not np.all(np.isfinite(values)):
        raise ValueError("Invalid solution vector")
    if not np.all(matrix @ values <= limits + 1e-7):
        raise ValueError(f"Original constraints violated: {values.tolist()}")
    costs = np.zeros(len(values))
    for term in model["objective"]["linear"]:
        costs[term["variable"]] += term["coefficient"]
    recomputed = float(costs @ values + model["objective"]["offset"])
    if abs(recomputed - objective) > 1e-6 * max(1.0, abs(recomputed)):
        raise ValueError(f"Published objective {objective} differs from original-model value {recomputed}")
    return {"objective": objective, "oracle_objective": optimum}


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("executable", type=Path)
    parser.add_argument("--cases", type=int, default=30)
    parser.add_argument("--report", type=Path)
    args = parser.parse_args()
    if not args.executable.is_file():
        parser.error("Executable does not exist")
    if not 1 <= args.cases <= 200:
        parser.error("Case count must lie between 1 and 200")
    rng = random.Random(SEED)
    outcomes, failures = [], []
    for i in range(args.cases):
        model = generated_model(i, rng)
        try:
            optimum, vertices = oracle(model)
            outcome = check_process(args.executable, model, optimum)
            outcomes.append({"case": i, "feasible_active_sets": vertices, **outcome})
        except (ValueError, KeyError, subprocess.TimeoutExpired) as error:
            failures.append({"case": i, "error": str(error), "model": model})
    report = {
        "oracle": "Independent active-constraint vertex enumeration; NumPy linear solves",
        "executed_at_utc": datetime.now(timezone.utc).isoformat(),
        "python_version": platform.python_version(),
        "platform": platform.platform(),
        "numpy_version": np.__version__, "seed": SEED, "case_count": args.cases,
        "passed": len(outcomes), "failed": len(failures),
        "scope": "Synthetic bounded continuous LPs only; no general solver-correctness proof",
        "results": outcomes, "failures": failures,
    }
    with args.executable.open("rb") as executable:
        report["executable_sha256"] = hashlib.file_digest(executable, "sha256").hexdigest()
    identity = subprocess.run(
        [str(args.executable), "simulate", "-"],
        input=json.dumps({"inputs": [{"kind": "constant", "value": 0}], "options": {"trials": 1}}),
        text=True, capture_output=True, timeout=20, check=True,
    )
    identity_payload = json.loads(identity.stdout)
    report["solvercraft_version"] = identity_payload["version"]
    report["sampling_version"] = identity_payload["result"]["sampling_version"]
    if args.report:
        args.report.parent.mkdir(parents=True, exist_ok=True)
        args.report.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({key: report[key] for key in ["numpy_version", "case_count", "passed", "failed", "seed"]}))
    if failures:
        print(json.dumps(failures, indent=2), file=sys.stderr)
    return 0 if not failures else 1


if __name__ == "__main__":
    raise SystemExit(main())

