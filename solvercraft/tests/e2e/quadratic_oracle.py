"""Independent strictly convex QP oracle: enumerate KKT active sets with NumPy.

The executable uses an iterative optimization backend; this oracle directly
solves every tiny candidate KKT system and rechecks signs and original rows.
Synthetic fixed-seed inputs only. No SciPy or Solvercraft internal calls.
"""
from __future__ import annotations

import argparse
from datetime import datetime, timezone
import hashlib
import itertools
import json
import math
from pathlib import Path
import platform
import random
import subprocess

import numpy as np

SEED = 20261031


def fixture(index: int, rng: random.Random) -> tuple[dict, np.ndarray, np.ndarray, np.ndarray, np.ndarray, np.ndarray, np.ndarray]:
    n = 2 + index % 2
    r = np.array([[rng.randint(-3, 3) for _ in range(n)] for _ in range(n)], dtype=float)
    hessian = r.T @ r + 0.5 * np.eye(n)
    cost = np.array([rng.randint(-10, 10) for _ in range(n)], dtype=float)
    a = [np.eye(n)[i] for i in range(n)] + [-np.eye(n)[i] for i in range(n)]
    b = [2.5] * n + [1.5] * n
    rows = []
    equal_a = []
    equal_b = []
    for row in range(3):
        coefficients = np.array([rng.randint(-3, 3) for _ in range(n)], dtype=float)
        if not np.any(coefficients):
            coefficients[0] = 1.0
        scale = 10.0 ** rng.randint(-1, 1)
        coefficients *= scale
        rhs = float(rng.randint(0, 4)) * scale
        equality = index % 5 == 0 and row == 0
        if equality:
            rhs = 0.0
            equal_a.append(coefficients)
            equal_b.append(rhs)
        else:
            a.append(coefficients)
            b.append(rhs)
        rows.append({"name": f"row{row}", "terms": [
            {"variable": i, "coefficient": float(c)} for i, c in enumerate(coefficients) if c],
            "relation": "equal" if equality else "less_equal", "rhs": rhs})
    sign = -1.0 if index % 3 == 0 else 1.0
    quadratic = [{"left": i, "right": j, "coefficient": sign * float(hessian[i, j]) * (0.5 if i == j else 1.0)}
                 for i in range(n) for j in range(i, n) if hessian[i, j]]
    model = {"schema_version": 1, "name": f"numpy-kkt-{index}",
             "variables": [{"name": f"x{i}", "kind": "continuous", "lower": -1.5, "upper": 2.5} for i in range(n)],
             "objective": {"direction": "maximize" if sign == -1 else "minimize", "offset": 0.25 * index,
                           "linear": [{"variable": i, "coefficient": sign * float(c)} for i, c in enumerate(cost) if c],
                           "quadratic": quadratic}, "constraints": rows}
    return model, hessian, cost, np.array(a), np.array(b), np.array(equal_a).reshape(-1, n), np.array(equal_b)


def oracle(hessian: np.ndarray, cost: np.ndarray, a: np.ndarray, b: np.ndarray,
           equal_a: np.ndarray, equal_b: np.ndarray) -> tuple[np.ndarray, float]:
    n = len(cost)
    candidates = []
    for count in range(n - len(equal_b) + 1):
        for active in itertools.combinations(range(len(b)), count):
            selected_a = a[list(active)] if active else np.zeros((0, n))
            selected_b = b[list(active)] if active else np.zeros(0)
            matrix = np.vstack((equal_a, selected_a))
            rhs = np.concatenate((equal_b, selected_b))
            kkt = np.block([[hessian, matrix.T], [matrix, np.zeros((len(rhs), len(rhs)))]])
            try:
                solution = np.linalg.solve(kkt, np.concatenate((-cost, rhs)))
            except np.linalg.LinAlgError:
                continue
            x = solution[:n]
            if np.any(a @ x - b > 1e-8) or np.any(np.abs(equal_a @ x - equal_b) > 1e-8):
                continue
            if np.any(solution[n + len(equal_b):] < -1e-8):
                continue
            candidates.append((x, float(0.5 * x @ hessian @ x + cost @ x)))
    if not candidates:
        raise RuntimeError("No independently feasible KKT point found")
    return min(candidates, key=lambda candidate: candidate[1])


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("executable", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--cases", type=int, default=20)
    args = parser.parse_args()
    if not args.executable.is_file() or not 1 <= args.cases <= 100:
        parser.error("Existing executable and 1..100 cases required")
    rng = random.Random(SEED)
    results = []
    failures = []
    for index in range(args.cases):
        model, hessian, cost, a, b, equal_a, equal_b = fixture(index, rng)
        expected_x, working_expected = oracle(hessian, cost, a, b, equal_a, equal_b)
        sign = -1 if model["objective"]["direction"] == "maximize" else 1
        expected = sign * working_expected + model["objective"]["offset"]
        request = {"model": model, "options": {"seed": "42", "max_iterations": 100000, "max_evaluations": 100000,
                                                  "feasibility_tolerance": 1e-7, "optimality_tolerance": 1e-8}}
        process = subprocess.run([str(args.executable.resolve()), "solve", "-"], input=json.dumps(request),
                                 capture_output=True, text=True, timeout=30)
        item = {"case": index, "request": request, "oracle_values": expected_x.tolist(), "oracle_objective": expected}
        try:
            if process.returncode or process.stderr:
                raise ValueError(f"CLI error {process.returncode}: {process.stderr}")
            payload = json.loads(process.stdout)
            result = payload["result"]
            item["actual"] = payload
            if (result["termination"], result["feasibility"], result["optimality"]) != ("completed", "feasible", "optimal"):
                raise ValueError(f"QP status did not establish a valid optimum: {result}")
            x = np.array(result["values"])
            direct = math.fsum([model["objective"]["offset"], *[
                t["coefficient"] * x[t["variable"]] for t in model["objective"]["linear"]], *[
                t["coefficient"] * x[t["left"]] * x[t["right"]] for t in model["objective"]["quadratic"]]])
            residual = max(0.0, float(np.max(a @ x - b)), float(np.max(np.abs(equal_a @ x - equal_b))) if len(equal_b) else 0.0)
            error = abs(direct - expected)
            item.update({"objective_error": error, "original_max_violation": residual, "value_error": float(np.max(np.abs(x - expected_x)))})
            if error > 1e-6 * max(1.0, abs(expected)) or residual > 1e-7 or abs(result["objective"] - direct) > 1e-7:
                raise ValueError(f"Original-model numeric discrepancy: {item}")
            if result["best_bound"] is None or sign * (result["best_bound"] - expected) > 1e-7 * max(1.0, abs(expected)):
                raise ValueError(f"Global bound excludes the independently derived optimum: {result}")
            item["passed"] = True
        except (ValueError, KeyError, TypeError) as error:
            item.update({"passed": False, "failure": str(error)})
            failures.append(index)
        results.append(item)
    report = {"schema": "solvercraft.numpy-kkt-oracle.v1", "date_utc": datetime.now(timezone.utc).isoformat(),
              "seed": SEED, "python": platform.python_version(), "numpy": np.__version__, "platform": platform.platform(),
              "executable_sha256": hashlib.sha256(args.executable.read_bytes()).hexdigest(),
              "cases": results, "passed": len(results) - len(failures), "failed": len(failures)}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"passed": report["passed"], "failed": report["failed"], "failed_cases": failures, "output": str(args.output)}))
    return int(bool(failures))


if __name__ == "__main__":
    raise SystemExit(main())
