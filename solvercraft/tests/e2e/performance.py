"""Bounded process benchmarks with independent analytic acceptance checks.

Reports process startup+JSON+computation time, not a kernel-only timing or an
industry comparison. Requires psutil only for process/host memory measurement.
"""
from __future__ import annotations

import argparse
from datetime import datetime, timezone
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import random
import statistics
import subprocess
import tempfile
import time

import psutil


def linear_request() -> dict:
    variables = [{"name": f"x{i}", "lower": 0.0, "upper": 1.0, "kind": "continuous"} for i in range(100)]
    terms = [{"variable": i, "coefficient": float(100 - i)} for i in range(100)]
    rows = [{"name": "capacity", "terms": [{"variable": i, "coefficient": 1.0} for i in range(100)],
             "relation": "less_equal", "rhs": 50.0}]
    rng = random.Random(7001)
    for i in range(499):
        subset = sorted(rng.sample(range(100), 10 + i % 80))
        # Redundant given individual upper bounds and the total capacity.
        rows.append({"name": f"redundant_{i}", "terms": [{"variable": j, "coefficient": 1.0} for j in subset],
                     "relation": "less_equal", "rhs": float(min(len(subset), 50))})
    return {"model": {"schema_version": 1, "name": "performance-100x500",
                       "variables": variables, "constraints": rows,
                       "objective": {"direction": "maximize", "offset": 0.0, "linear": terms, "quadratic": []}},
            "options": {"seed": "42", "max_memory_bytes": 268435456,
                        "max_evaluations": 100000, "max_iterations": 100000}}


def simulation_request() -> dict:
    return {"inputs": [{"kind": "uniform", "low": 0.0, "high": 1.0} for _ in range(20)],
            "outputs": [{"op": "add", "left": {"op": "variable", "index": 2 * i},
                         "right": {"op": "variable", "index": 2 * i + 1}} for i in range(10)],
            "options": {"trials": 100000, "seed": "42", "method": "monte_carlo",
                        "retain_samples": False, "max_memory_bytes": 33554432}}


def run(executable: Path, command: str, request: dict, work: Path) -> tuple[dict, dict]:
    encoded = json.dumps(request, separators=(",", ":")).encode()
    with tempfile.TemporaryFile(dir=work) as output, tempfile.TemporaryFile(dir=work) as errors:
        start = time.perf_counter()
        process = subprocess.Popen([str(executable), command, "-"], stdin=subprocess.PIPE, stdout=output, stderr=errors)
        observer = psutil.Process(process.pid)
        process.stdin.write(encoded)
        process.stdin.close()
        peak_rss = 0
        peak_working_set = 0
        while process.poll() is None:
            try:
                memory = observer.memory_info()
                peak_rss = max(peak_rss, memory.rss)
                peak_working_set = max(peak_working_set, getattr(memory, "peak_wset", memory.rss))
            except psutil.NoSuchProcess:
                pass
            if time.perf_counter() - start > 120:
                process.kill()
                process.wait(timeout=5)
                raise RuntimeError("Benchmark child exceeded 120 seconds")
            time.sleep(0.005)
        elapsed = time.perf_counter() - start
        output.seek(0)
        errors.seek(0)
        stdout = output.read(1024 * 1024 + 1)
        stderr = errors.read(128 * 1024).decode(errors="replace")
        if process.returncode or len(stdout) > 1024 * 1024:
            raise RuntimeError(f"Benchmark failed: exit {process.returncode}, stderr {stderr}")
        envelope = json.loads(stdout)
        value = envelope["result"]
        return value, {"wall_seconds": elapsed, "peak_sampled_rss_bytes": peak_rss,
                       "os_peak_working_set_bytes": peak_working_set,
                       "request_sha256": hashlib.sha256(encoded).hexdigest(), "stderr": stderr,
                       "run_identity": {key: item for key, item in envelope.items() if key != "result"}}


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("executable", type=Path)
    parser.add_argument("--repeat", type=int, default=5)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--work-dir", type=Path, required=True)
    args = parser.parse_args()
    if not 1 <= args.repeat <= 20 or not args.executable.is_file():
        parser.error("Need an existing executable and 1..20 repetitions")
    args.work_dir.mkdir(parents=True, exist_ok=True)
    report = {"schema": "solvercraft.performance.v1", "started_utc": datetime.now(timezone.utc).isoformat(),
              "executable": str(args.executable.resolve()),
              "executable_sha256": hashlib.sha256(args.executable.read_bytes()).hexdigest(),
              "host": {"platform": platform.platform(), "architecture": platform.machine(),
                       "processor": os.environ.get("PROCESSOR_IDENTIFIER", platform.processor()),
                       "logical_cpus": psutil.cpu_count(), "physical_cpus": psutil.cpu_count(logical=False),
                       "ram_bytes": psutil.virtual_memory().total, "python": platform.python_version(), "psutil": psutil.__version__},
              "timing": "Every measured sample is a new process; includes startup, JSON and computation. No warmup discarded.",
              "memory": "5ms RSS sampling plus Windows OS peak working set when available; not an allocation-level proof.",
              "host_load_note": "Shared development host; other builds may run concurrently. Baseline observations, not an isolated product comparison.",
              "cases": []}
    for name, command, request in (("continuous_lp_100_variables_500_rows", "solve", linear_request()),
                                    ("streaming_mc_100000_trials_20_inputs_10_outputs", "simulate", simulation_request())):
        measurements = []
        for _ in range(args.repeat):
            value, measurement = run(args.executable.resolve(), command, request, args.work_dir)
            if command == "solve":
                expected = 3775.0  # Sum of the 50 largest profits, 51..100.
                if value["termination"] != "completed" or value["optimality"] != "optimal" or value["feasibility"] != "feasible":
                    raise RuntimeError(f"LP did not prove optimality: {value}")
                values = value["values"]
                violation = max([0.0, sum(values) - 50, *[-x for x in values], *[x - 1 for x in values]])
                for row in request["model"]["constraints"]:
                    violation = max(violation, sum(values[t["variable"]] * t["coefficient"] for t in row["terms"]) - row["rhs"])
                error = abs(value["objective"] - expected)
                direct = sum(x * (100 - i) for i, x in enumerate(values))
                if error > 1e-6 or violation > 1e-7 or abs(direct - expected) > 1e-6:
                    raise RuntimeError(f"Independent LP acceptance failed: objective error {error}, residual {violation}")
                measurement.update({"objective_error": error, "original_max_violation": violation,
                                    "engine": value["engine"], "statistics": value["statistics"]})
            else:
                if value["termination"] != "completed" or value["completed_trials"] != 100000 or value["samples"] is not None:
                    raise RuntimeError("Streaming simulation did not complete with the requested storage policy")
                summaries = value["summaries"]
                if len(summaries) != 10:
                    raise RuntimeError("Incorrect output count")
                error = max(abs(item["mean"] - 1.0) for item in summaries)
                variance_error = max(abs(item["variance"] - 1.0 / 6.0) for item in summaries)
                if any(item["count"] != 100000 for item in summaries) or error > 6 * math.sqrt(1 / 600000) or variance_error > 0.005:
                    raise RuntimeError(f"Analytic simulation check failed: mean {error}, variance {variance_error}")
                measurement.update({"max_mean_absolute_error": error, "max_variance_absolute_error": variance_error,
                                    "sampling_version": value["sampling_version"]})
            measurements.append(measurement)
        report["cases"].append({"name": name, "request": request, "measurements": measurements,
                                 "median_wall_seconds": statistics.median(m["wall_seconds"] for m in measurements),
                                 "max_os_peak_working_set_bytes": max(m["os_peak_working_set_bytes"] for m in measurements)})
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"output": str(args.output), "cases": [
        {"name": c["name"], "median_wall_seconds": c["median_wall_seconds"],
         "peak_working_set_bytes": c["max_os_peak_working_set_bytes"]} for c in report["cases"]]}))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
