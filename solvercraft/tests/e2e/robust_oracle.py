"""Independent process oracle: original-space uncertainty vertices, no auxiliaries.

Bounded two-dimensional box and cardinality-budget models are expanded to their
actual uncertainty-set vertices. NumPy enumerates the resulting LP vertices.
Tiny-fixed-variable cases separately require honest rejection of false robust
feasibility; their infeasibility is analytic, not judged with LP bound tolerances.
"""
from __future__ import annotations

import argparse
import copy
from datetime import datetime, timezone
import hashlib
import itertools
import json
import platform
from pathlib import Path
import random
import subprocess

import numpy as np
from linear_oracle import inequalities, oracle

SEED = 20261031


def uncertainty_vertices(kind: str, gamma: float) -> list[np.ndarray]:
    if kind == "box":
        return [np.asarray(values, dtype=float) for values in itertools.product((-1, 1), repeat=2)]
    # Intersections of the unit square with all four |u1|+|u2| inequalities.
    rows = np.asarray([[1, 0], [-1, 0], [0, 1], [0, -1], [1, 1], [1, -1], [-1, 1], [-1, -1]], dtype=float)
    limits = np.asarray([1, 1, 1, 1, gamma, gamma, gamma, gamma], dtype=float)
    vertices = {}
    for pair in itertools.combinations(range(8), 2):
        try:
            point = np.linalg.solve(rows[list(pair)], limits[list(pair)])
        except np.linalg.LinAlgError:
            continue
        if np.all(rows @ point <= limits + 1e-12):
            vertices[tuple(np.round(point, 12))] = point
    if not vertices:
        raise ValueError("No uncertainty vertices")
    return list(vertices.values())


def expanded_model(model: dict, uncertainties: list[dict], kind: str) -> dict:
    expanded = copy.deepcopy(model)
    expanded["constraints"] = []
    for row, uncertainty in zip(model["constraints"], uncertainties):
        nominal = np.zeros(2)
        for term in row["terms"]:
            nominal[term["variable"]] += term["coefficient"]
        for index, vertex in enumerate(uncertainty_vertices(kind, uncertainty.get("budget", 2.0))):
            coefficients = nominal + np.asarray(uncertainty["radii"]) * vertex
            expanded["constraints"].append({"name": f"{row['name']}.vertex{index}", "relation": "less_equal", "rhs": row["rhs"],
                "terms": [{"variable": i, "coefficient": float(value)} for i, value in enumerate(coefficients) if value != 0.0]})
    return expanded


def request(index: int, kind: str, rng: random.Random) -> dict:
    gamma = [0.0, 0.5, 1.0, 1.5, 2.0][index % 5]
    model = {"schema_version": 1, "name": f"uncertainty-vertices-{kind}-{index}",
        "variables": [{"name": "x", "lower": -3.0, "upper": 2.0, "kind": "continuous"},
                      {"name": "y", "lower": -2.0, "upper": 4.0, "kind": "continuous"}],
        "objective": {"direction": "minimize" if index % 2 else "maximize", "offset": 0.25,
                      "linear": [{"variable": 0, "coefficient": float(rng.choice([-3, -1, 1, 3]))},
                                 {"variable": 1, "coefficient": float(rng.choice([-4, -2, 2, 4]))}], "quadratic": []},
        "constraints": [{"name": f"r{j}", "relation": "less_equal", "rhs": float(rng.randint(1, 6)),
            "terms": [{"variable": i, "coefficient": float(rng.randint(-3, 3))} for i in range(2)]} for j in range(2)]}
    uncertainty = [{"constraint": j, "radii": [rng.randint(0, 5) / 2.0, rng.randint(1, 5) / 2.0]} for j in range(2)]
    if kind == "budget":
        for item in uncertainty:
            item["budget"] = gamma
    return {"model": model, "uncertainties": uncertainty, "options": {"feasibility_tolerance": 1e-8, "seed": str(SEED)}}


def process(executable: Path, kind: str, data: dict) -> subprocess.CompletedProcess:
    return subprocess.run([str(executable), "robust-" + kind, "-"], input=json.dumps(data),
                          capture_output=True, text=True, timeout=20, check=False)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("executable", type=Path)
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args()
    rng = random.Random(SEED)
    records = []
    for kind in ("box", "budget"):
        for index in range(10):
            data = request(index, kind, rng)
            original_vertices = expanded_model(data["model"], data["uncertainties"], kind)
            optimum, count = oracle(original_vertices)
            response = process(args.executable, kind, data)
            if response.returncode or response.stderr:
                raise ValueError(f"{kind}{index}: {response.stderr}")
            result = json.loads(response.stdout)["result"]
            if (result["termination"], result["feasibility"], result["optimality"]) != ("completed", "feasible", "optimal"):
                raise ValueError(f"{kind}{index}: invalid status {result}")
            values = np.asarray(result["values"][:2], dtype=float)
            rows, bounds = inequalities(original_vertices)
            violation = float(max(0.0, np.max(rows @ values - bounds)))
            error = abs(result["objective"] - optimum)
            if violation > 1e-7 or error > 1e-6 * (1.0 + abs(optimum)):
                raise ValueError(f"{kind}{index}: original uncertainty violation{violation}, objective error{error}")
            costs = np.zeros(2)
            for term in data["model"]["objective"]["linear"]:
                costs[term["variable"]] += term["coefficient"]
            direct_objective = float(costs @ values + data["model"]["objective"]["offset"])
            if abs(direct_objective - result["objective"]) > 1e-7:
                raise ValueError("Reported objective differs from original variables")
            records.append({"kind": kind, "request": data, "oracle_objective": optimum, "objective": result["objective"],
                            "values": values.tolist(), "feasible_vertices": count, "original_vertex_violation": violation, "objective_error": error})
    tiny = []
    for kind in ("box", "budget"):
        for sign in (-1.0, 1.0):
            value = sign * 1e-9
            model = {"schema_version": 1, "name": "tiny-fixed-original-support", "variables": [
                {"name": "x", "lower": value, "upper": value, "kind": "continuous"}],
                "objective": {"direction": "minimize", "offset": 0.0, "linear": [], "quadratic": []},
                "constraints": [{"name": "support", "terms": [], "relation": "less_equal", "rhs": 0.0}]}
            uncertainty = {"constraint": 0, "radii": [1e12]}
            if kind == "budget":
                uncertainty["budget"] = 0.5
            data = {"model": model, "uncertainties": [uncertainty], "options": {"feasibility_tolerance": 1e-8}}
            response = process(args.executable, kind, data)
            if response.returncode:
                error = json.loads(response.stderr)
                if response.stdout or not error.get("code"):
                    raise ValueError("Expected exactly one structured failure")
                status = {"error": error["code"]}
            else:
                if response.stderr:
                    raise ValueError("Unexpected stderr")
                result = json.loads(response.stdout)["result"]
                if result["feasibility"] == "feasible" or result["optimality"] == "optimal":
                    raise ValueError(f"False robust feasibility: original support{1e12 * abs(value)}: {result}")
                status = {"termination": result["termination"], "feasibility": result["feasibility"], "optimality": result["optimality"]}
            tiny.append({"kind": kind, "fixed_value": value, "analytic_worst_case": 1000.0 if kind == "box" else 500.0, "outcome": status})
    report = {"oracle": "explicit uncertainty vertices + independent NumPy LP vertex enumeration", "seed": str(SEED),
        "executable_sha256": hashlib.sha256(args.executable.read_bytes()).hexdigest(), "numpy": np.__version__, "platform": platform.platform(),
        "date_utc": datetime.now(timezone.utc).isoformat(), "ordinary_cases_passed": len(records), "tiny_infeasible_cases_passed": len(tiny),
        "cases": records, "tiny_cases": tiny}
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({key: value for key, value in report.items() if key not in ("cases", "tiny_cases")}))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
