"""Reproduce independent Johnson population fixtures using Gaussian quadrature.

Requires NumPy only. Uses 128 and 256 Hermite nodes and compares convergence;
does not import Solvercraft, sample a distribution or use its fitting formulas.
The normal translation gives all three reference quantiles at z=-1,0,+1.
"""
from __future__ import annotations

import argparse
import json
from pathlib import Path

import numpy as np
from numpy.polynomial.hermite import hermgauss

CASES = [
    ("su", 0.0, 30.0, 1e6, 100.0),
    ("su", -1.5, 3.0, -7.0, 4.0),
    ("sb", 0.0, 30.0, 1e6, 100.0),
    ("sb", 6.0, 2.0, -7.0, 4.0),
    ("sb", -4.0, 2.0, 7.0, 0.25),
    ("su", 0.0, 1.0, 3.0, 2.0),
]


def translation(family: str, gamma: float, delta: float, z: np.ndarray) -> np.ndarray:
    u = (z - gamma) / delta
    return np.sinh(u) if family == "su" else 1.0 / (1.0 + np.exp(-u))


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--report", type=Path)
    args = parser.parse_args()
    records = []
    for family, gamma, delta, location, scale in CASES:
        estimates = []
        for count in (128, 256):
            nodes, weights = hermgauss(count)
            weights /= np.sqrt(np.pi)
            y = translation(family, gamma, delta, np.sqrt(2.0) * nodes)
            mean = weights @ y
            centered = y - mean
            variance = weights @ (centered ** 2)
            estimates.append(np.array([
                location + scale * mean, scale * np.sqrt(variance),
                (weights @ (centered ** 3)) / variance ** 1.5,
                (weights @ (centered ** 4)) / variance ** 2,
            ]))
        convergence = float(np.max(np.abs(estimates[1] - estimates[0]) / (1.0 + np.abs(estimates[1]))))
        if convergence > 1e-12:
            raise ValueError(f"Quadrature did not converge for {family}, gamma={gamma}, delta={delta}")
        quantiles = location + scale * translation(family, gamma, delta, np.array([-1.0, 0.0, 1.0]))
        records.append({"family": family, "gamma": gamma, "delta": delta,
                        "location": location, "scale": scale,
                        "moments": estimates[-1].tolist(), "quantiles_z_minus1_0_plus1": quantiles.tolist(),
                        "maximum_relative_quadrature_difference": convergence})
    report = {"oracle": "independent NumPy Gauss-Hermite 128/256", "numpy": np.__version__,
              "kurtosis_convention": "Pearson", "cases": records,
              "symmetry_note": "Symmetric fixtures use exactly zero skewness in Rust acceptance assertions."}
    rendered = json.dumps(report, indent=2) + "\n"
    if args.report:
        args.report.parent.mkdir(parents=True, exist_ok=True)
        args.report.write_text(rendered, encoding="utf-8")
    print(rendered)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
