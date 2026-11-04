//! Finite deterministic adversarial matrices. Oracles use integer determinants,
//! exhaustive integer slices, KKT projections, and Euclidean support geometry;
//! no solver output is used to manufacture an expected answer.
#![allow(clippy::unwrap_used, clippy::expect_used)]
use solvercraft_engines::{AffineExpression, ConicModel, SecondOrderConeConstraint, solve, solve_owned, solve_socp_owned, solve_sparse_owned};
use solvercraft_types::*;
use std::sync::Arc;

#[derive(Clone, Copy, Debug)]
struct Row {
    a: i64,
    b: i64,
    rhs: i64,
}
#[derive(Clone, Copy, Debug)]
struct Point {
    x: i128,
    y: i128,
    d: i128,
}
impl Point {
    fn feasible(self, rows: &[Row]) -> bool {
        rows.iter().all(|r| i128::from(r.a) * self.x + i128::from(r.b) * self.y <= i128::from(r.rhs) * self.d)
    }
    fn score(self, c: [i64; 2]) -> i128 {
        i128::from(c[0]) * self.x + i128::from(c[1]) * self.y
    }
}
fn vertices(rows: &[Row]) -> Vec<Point> {
    let mut out = Vec::new();
    for (i, a) in rows.iter().enumerate() {
        for b in &rows[i + 1..] {
            let d = i128::from(a.a) * i128::from(b.b) - i128::from(a.b) * i128::from(b.a);
            if d == 0 {
                continue;
            }
            let p = Point {
                x: (i128::from(a.rhs) * i128::from(b.b) - i128::from(a.b) * i128::from(b.rhs)) * d.signum(),
                y: (i128::from(a.a) * i128::from(b.rhs) - i128::from(a.rhs) * i128::from(b.a)) * d.signum(),
                d: d.abs(),
            };
            if p.feasible(rows) {
                out.push(p);
            }
        }
    }
    out
}
fn oracle(rows: &[Row], c: [i64; 2], direction: Direction, kinds: [VariableKind; 2]) -> Option<f64> {
    let mut points = Vec::new();
    if kinds[0] == VariableKind::Continuous {
        points = vertices(rows);
    } else {
        // Every generated box is contained in [-3,4] x [-2,5]. Fixing the
        // integer x leaves a one-dimensional rational LP, whose endpoints are
        // enumerated exactly by intersecting its supporting lines.
        for x in -3..=4 {
            let mut slice = rows.to_vec();
            slice.extend([Row { a: 1, b: 0, rhs: x }, Row { a: -1, b: 0, rhs: -x }]);
            for p in vertices(&slice) {
                if kinds[1] == VariableKind::Continuous || p.y % p.d == 0 {
                    points.push(p);
                }
            }
            if kinds[1] != VariableKind::Continuous {
                for y in -2..=5 {
                    let p = Point { x: i128::from(x), y: i128::from(y), d: 1 };
                    if p.feasible(&slice) {
                        points.push(p);
                    }
                }
            }
        }
    }
    let mut best: Option<Point> = None;
    for p in points {
        let better = best.is_none_or(|v| {
            let cmp = (p.score(c) * v.d).cmp(&(v.score(c) * p.d));
            if direction == Direction::Minimize { cmp.is_lt() } else { cmp.is_gt() }
        });
        if better {
            best = Some(p);
        }
    }
    best.map(|p| p.score(c) as f64 / p.d as f64)
}
struct Rng(u64);
impl Rng {
    fn next(&mut self, n: u64) -> u64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.0 >> 32) % n
    }
    fn signed(&mut self, radius: i64) -> i64 {
        self.next((2 * radius + 1) as u64) as i64 - radius
    }
}
fn terms(c: [f64; 2], duplicate: bool, swap: bool) -> Vec<LinearTerm> {
    let mut out = Vec::new();
    for (i, coefficient) in c.into_iter().enumerate() {
        let variable = if swap { 1 - i } else { i };
        if duplicate {
            // Deliberate cancellation as well as exact split coefficients.
            out.extend([
                LinearTerm { variable, coefficient: 8.0 },
                LinearTerm { variable, coefficient: coefficient / 2.0 },
                LinearTerm { variable, coefficient: -8.0 },
                LinearTerm { variable, coefficient: coefficient / 2.0 },
            ]);
        } else {
            out.push(LinearTerm { variable, coefficient });
        }
    }
    out
}
struct Case {
    model: OptimizationModel,
    rows: Vec<Row>,
    c: [i64; 2],
    kinds: [VariableKind; 2],
    scale: f64,
    offset: f64,
    swap: bool,
}
fn linear_case(seed: u64, integer: bool) -> Case {
    let mut rng = Rng(seed ^ 0x9e3779b97f4a7c15);
    let binary = integer && seed.is_multiple_of(7);
    let kinds = if binary {
        [VariableKind::Binary; 2]
    } else if integer {
        [VariableKind::Integer, if seed.is_multiple_of(2) { VariableKind::Integer } else { VariableKind::Continuous }]
    } else {
        [VariableKind::Continuous; 2]
    };
    let lower = if binary { [0, 0] } else { [-3, -2] };
    let mut upper = if binary { [1, 1] } else { [4, 5] };
    if seed.is_multiple_of(11) {
        upper[0] = lower[0];
    }
    let mut rows = vec![
        Row { a: -1, b: 0, rhs: -lower[0] },
        Row { a: 1, b: 0, rhs: upper[0] },
        Row { a: 0, b: -1, rhs: -lower[1] },
        Row { a: 0, b: 1, rhs: upper[1] },
    ];
    for _ in 0..2 + seed % 5 {
        let mut a = rng.signed(3);
        let b = rng.signed(3);
        if a == 0 && b == 0 {
            a = 1;
        }
        rows.push(Row { a, b, rhs: rng.signed(9) });
    }
    // Exact redundant rows exercise rank deficiency and degeneracy.
    if seed.is_multiple_of(3) {
        rows.push(rows[4]);
    }
    if seed.is_multiple_of(5) {
        rows.push(Row { a: 0, b: 0, rhs: 0 });
    }
    let mut c = [rng.signed(4), rng.signed(4)];
    if c == [0, 0] {
        c[0] = 1;
    }
    let direction = if seed.is_multiple_of(2) { Direction::Minimize } else { Direction::Maximize };
    let scale = [0.125, 0.5, 1.0, 8.0][seed as usize % 4];
    let offset = rng.signed(100) as f64 / 4.0;
    let swap = seed % 3 == 1;
    let mut variables = Vec::new();
    let mut constraints = Vec::new();
    for i in 0_usize..2 {
        let mode = (seed as usize + i) % 4;
        variables.push(Variable {
            name: format!("v{i}"),
            kind: kinds[i],
            lower: if mode < 2 { Some(lower[i] as f64) } else { None },
            upper: if mode.is_multiple_of(2) { Some(upper[i] as f64) } else { None },
        });
    }
    for (i, r) in rows.iter().enumerate() {
        if i < 4 {
            let variable = i / 2;
            let mode = (seed as usize + variable) % 4;
            if i.is_multiple_of(2) && mode < 2 || i % 2 == 1 && mode.is_multiple_of(2) {
                continue;
            }
        }
        let row_scale = [0.25, 1.0, 16.0][(seed as usize + i) % 3];
        let flip = i.is_multiple_of(2);
        let sign = if flip { -1.0 } else { 1.0 };
        constraints.push(LinearConstraint {
            name: format!("r{i}"),
            terms: terms([r.a as f64 * sign * row_scale, r.b as f64 * sign * row_scale], seed.is_multiple_of(4), swap),
            relation: if flip { Relation::GreaterEqual } else { Relation::LessEqual },
            rhs: r.rhs as f64 * sign * row_scale,
        });
    }
    if seed.is_multiple_of(2) {
        constraints.reverse();
    }
    if swap {
        variables.reverse();
    }
    let model = OptimizationModel {
        schema_version: SCHEMA_VERSION,
        name: format!("oracle-seed-{seed}-integer-{integer}"),
        variables,
        objective: Objective {
            direction,
            offset,
            linear: terms([scale * c[0] as f64, scale * c[1] as f64], seed.is_multiple_of(4), swap),
            quadratic: vec![],
        },
        constraints,
    };
    Case { model, rows, c, kinds, scale, offset, swap }
}
fn check_linear(case: &Case, result: &SolveResult) -> std::result::Result<(), String> {
    let expected = oracle(&case.rows, case.c, case.model.objective.direction, case.kinds).map(|v| case.offset + case.scale * v);
    if expected.is_none() {
        if result.termination == Termination::Completed && result.feasibility == Feasibility::Infeasible {
            return Ok(());
        }
        return Err(format!("expected exact infeasible: {result:?}"));
    }
    let expected = expected.unwrap();
    if result.termination != Termination::Completed || result.feasibility != Feasibility::Feasible || result.optimality != Optimality::Optimal {
        return Err(format!("expected optimal {expected}: {result:?}"));
    }
    let values = result.values.as_ref().ok_or("missing values")?;
    let p = if case.swap { [values[1], values[0]] } else { [values[0], values[1]] };
    for r in &case.rows {
        let residual = r.a as f64 * p[0] + r.b as f64 * p[1] - r.rhs as f64;
        if residual > 2e-6 {
            return Err(format!("original row {r:?} residual {residual}: {result:?}"));
        }
    }
    for (value, kind) in p.into_iter().zip(case.kinds) {
        if kind != VariableKind::Continuous && (value - value.round()).abs() > 1e-7 {
            return Err(format!("noninteger {value}"));
        }
    }
    let direct = case.offset + case.scale * (case.c[0] as f64 * p[0] + case.c[1] as f64 * p[1]);
    for (label, value) in [("objective", result.objective), ("bound", result.best_bound)] {
        let value = value.ok_or(format!("missing {label}"))?;
        if (value - expected).abs() > 2e-6 {
            return Err(format!("{label}={value}, exact={expected}: {result:?}"));
        }
    }
    if (direct - expected).abs() > 2e-6 {
        return Err(format!("direct={direct}, exact={expected}: {result:?}"));
    }
    Ok(())
}
fn finish(label: &str, count: usize, failures: Vec<String>) {
    eprintln!("MATRIX {label}: cases={count}, failures={}", failures.len());
    if std::env::var_os("SOLVERCRAFT_MATRIX_DIAGNOSTICS").is_some() {
        for failure in &failures {
            eprintln!("MATRIX FAILURE {label}: {failure}");
        }
    }
    assert!(failures.is_empty(), "{} failures in {label}:\n{}", failures.len(), failures.iter().take(12).cloned().collect::<Vec<_>>().join("\n"));
}
#[test]
fn exact_vertex_lp_matrix_dense_and_sparse() {
    let mut failures = Vec::new();
    for seed in 0..1024 {
        let case = linear_case(seed, false);
        for sparse in [false, true] {
            let result = if sparse {
                solve_sparse_owned(&case.model, &SolveOptions::default(), Arc::new(NoControl)).map(|v| v.result)
            } else {
                solve(&case.model, &SolveOptions::default(), &NoControl)
            };
            let check = result.map_err(|e| e.to_string()).and_then(|r| check_linear(&case, &r));
            if let Err(e) = check {
                failures.push(format!("seed={seed}, sparse={sparse}: {e}"));
            }
        }
    }
    finish("exact LP vertices", 2048, failures);
}
#[test]
fn exact_integer_slice_milp_matrix() {
    let mut failures = Vec::new();
    for seed in 0..768 {
        let case = linear_case(seed, true);
        let check = solve(&case.model, &SolveOptions::default(), &NoControl).map_err(|e| e.to_string()).and_then(|r| check_linear(&case, &r));
        if let Err(e) = check {
            failures.push(format!("seed={seed}: {e}"));
        }
    }
    finish("exact MILP slices", 768, failures);
}

fn quadratic_case(i: usize) -> (OptimizationModel, [f64; 2], f64) {
    // Metric projection of center a onto n*x >= n*a+gap. The KKT solution
    // is a+gap*Q^-1*n/(n'Q^-1*n); its value is gap^2/(n'Q^-1*n).
    let a = [(i % 11) as f64 / 4.0 - 1.25, (i % 13) as f64 / 4.0 - 1.5];
    let q = [1.0 + (i % 3) as f64, 1.0 + (i % 5) as f64];
    let n = [1.0 + (i % 4) as f64, -1.0 - (i % 3) as f64];
    let gap = (i % 7) as f64 / 4.0;
    let denominator = n[0] * n[0] / q[0] + n[1] * n[1] / q[1];
    let expected = [a[0] + gap * n[0] / q[0] / denominator, a[1] + gap * n[1] / q[1] / denominator];
    let sign = if i.is_multiple_of(2) { 1.0 } else { -1.0 };
    let offset = (i % 17) as f64 / 4.0 - 2.0;
    let mut model = OptimizationModel {
        schema_version: SCHEMA_VERSION,
        name: format!("KKT-{i}"),
        variables: (0..2)
            .map(|j| Variable { name: format!("x{j}"), lower: Some(a[j] - 8.0), upper: Some(a[j] + 8.0), kind: VariableKind::Continuous })
            .collect(),
        objective: Objective {
            direction: if sign > 0.0 { Direction::Minimize } else { Direction::Maximize },
            offset: offset + sign * (q[0] * a[0] * a[0] + q[1] * a[1] * a[1]),
            linear: terms([-2.0 * sign * q[0] * a[0], -2.0 * sign * q[1] * a[1]], i.is_multiple_of(3), false),
            quadratic: (0..2).map(|j| QuadraticTerm { left: j, right: j, coefficient: sign * q[j] }).collect(),
        },
        constraints: vec![LinearConstraint {
            name: "projection".into(),
            terms: terms(n, i.is_multiple_of(3), false),
            relation: if i.is_multiple_of(5) { Relation::Equal } else { Relation::GreaterEqual },
            rhs: n[0] * a[0] + n[1] * a[1] + gap,
        }],
    };
    if i.is_multiple_of(4) {
        model.constraints.push(model.constraints[0].clone());
    }
    if i.is_multiple_of(6) {
        for term in model.objective.quadratic.clone() {
            model.objective.quadratic.push(QuadraticTerm { coefficient: -term.coefficient / 2.0, ..term });
            model.objective.quadratic.push(QuadraticTerm { coefficient: term.coefficient / 2.0, ..term });
        }
    }
    (model, expected, offset + sign * gap * gap / denominator)
}
fn check_optimum(model: &OptimizationModel, result: &SolveResult, expected: f64, point: Option<&[f64]>) -> std::result::Result<(), String> {
    if result.termination != Termination::Completed || result.feasibility != Feasibility::Feasible || result.optimality != Optimality::Optimal {
        return Err(format!("expected optimum {expected}: {result:?}"));
    }
    let values = result.values.as_ref().ok_or("missing point")?;
    let objective = result.objective.ok_or("missing objective")?;
    if (objective - expected).abs() > 2e-6 {
        return Err(format!("objective {objective} != {expected}: {result:?}"));
    }
    if let Some(point) = point
        && values.iter().zip(point).any(|(a, b)| (a - b).abs() > 2e-5)
    {
        return Err(format!("point != {point:?}: {result:?}"));
    }
    let bound = result.best_bound.ok_or("missing optimal bound")?;
    let invalid = if model.objective.direction == Direction::Minimize { bound > expected + 1e-7 } else { bound < expected - 1e-7 };
    if invalid || (bound - expected).abs() > 2e-6 {
        return Err(format!("bound {bound} != {expected}: {result:?}"));
    }
    Ok(())
}
#[test]
fn constrained_qp_kkt_matrix_dense_and_sparse() {
    let mut failures = Vec::new();
    for i in 0_usize..256 {
        let (model, point, expected) = quadratic_case(i);
        for sparse in [false, true] {
            let result = if sparse {
                solve_sparse_owned(&model, &SolveOptions::default(), Arc::new(NoControl)).map(|v| v.result)
            } else {
                solve(&model, &SolveOptions::default(), &NoControl)
            };
            let check = result.map_err(|e| e.to_string()).and_then(|r| check_optimum(&model, &r, expected, Some(&point)));
            if let Err(e) = check {
                failures.push(format!("KKT={i}, sparse={sparse}: {e}"));
            }
        }
    }
    finish("constrained QP KKT", 512, failures);
}
#[test]
fn finite_miqp_matches_exhaustive_original_polynomial() {
    let mut failures = Vec::new();
    for i in 0_usize..128 {
        let a = (i % 11) as f64 / 4.0 - 1.25;
        let b = (i % 7) as f64 / 4.0 - 0.75;
        let d = (i % 5) as f64 / 4.0;
        let k = 1.0 + (i % 3) as f64;
        let q = 1.0 + (i % 4) as f64;
        let offset = (i % 17) as f64 / 4.0;
        let sign = if i.is_multiple_of(2) { 1.0 } else { -1.0 };
        let mut model = OptimizationModel {
            schema_version: SCHEMA_VERSION,
            name: format!("enumerated-MIQP-{i}"),
            variables: (0..2).map(|j| Variable { name: format!("x{j}"), lower: Some(-3.0), upper: Some(3.0), kind: VariableKind::Integer }).collect(),
            objective: Objective {
                direction: if sign > 0.0 { Direction::Minimize } else { Direction::Maximize },
                offset: offset + sign * (a * a + q * b * b + k * d * d),
                linear: terms([sign * (-2.0 * a - 2.0 * k * d), sign * (-2.0 * q * b + 2.0 * k * d)], i.is_multiple_of(3), false),
                quadratic: vec![
                    QuadraticTerm { left: 0, right: 0, coefficient: sign * (1.0 + k) },
                    QuadraticTerm { left: 1, right: 1, coefficient: sign * (q + k) },
                    QuadraticTerm { left: 0, right: 1, coefficient: -2.0 * sign * k },
                ],
            },
            constraints: vec![LinearConstraint {
                name: "coupling".into(),
                terms: terms([1.0, 2.0], false, false),
                relation: Relation::GreaterEqual,
                rhs: (i % 9) as f64 - 4.0,
            }],
        };
        if i.is_multiple_of(5) {
            model.variables[1].kind = VariableKind::Binary;
            model.variables[1].lower = Some(0.0);
            model.variables[1].upper = Some(1.0);
        }
        let mut best = f64::INFINITY;
        for x in -3..=3 {
            for y in -3..=3 {
                if i.is_multiple_of(5) && !(0..=1).contains(&y) || ((x + 2 * y) as f64) < model.constraints[0].rhs {
                    continue;
                }
                // All values are dyadic and small: these operations are exact.
                let value = (x as f64 - a).powi(2) + q * (y as f64 - b).powi(2) + k * (x as f64 - y as f64 - d).powi(2);
                best = best.min(value);
            }
        }
        let expected = offset + sign * best;
        let check = solve_owned(&model, &SolveOptions::default(), Arc::new(NoControl))
            .map_err(|e| e.to_string())
            .and_then(|r| check_optimum(&model, &r, expected, None));
        if let Err(e) = check {
            failures.push(format!("MIQP={i}: {e}"));
        }
    }
    finish("enumerated MIQP", 128, failures);
}
#[test]
fn ellipsoid_socp_support_geometry_matrix() {
    let mut failures = Vec::new();
    for i in 0_usize..128 {
        let n = 2 + i % 4;
        let radius = 0.5 + (i % 7) as f64 / 4.0;
        let center = (0..n).map(|j| ((i + j) % 9) as f64 / 4.0 - 1.0).collect::<Vec<_>>();
        let axes = (0..n).map(|j| [0.5, 1.0, 2.0][(i + j) % 3]).collect::<Vec<_>>();
        let c = (0..n).map(|j| 1.0 + ((3 * i + j) % 5) as f64).collect::<Vec<_>>();
        let length = c.iter().zip(&axes).map(|(c, a)| (c * a).powi(2)).sum::<f64>().sqrt();
        let sign = if i.is_multiple_of(2) { 1.0 } else { -1.0 };
        let offset = (i % 11) as f64 / 4.0;
        let expected = offset + sign * (center.iter().zip(&c).map(|(a, b)| a * b).sum::<f64>() - radius * length);
        let point = (0..n).map(|j| center[j] - radius * axes[j] * axes[j] * c[j] / length).collect::<Vec<_>>();
        let model = ConicModel {
            base: OptimizationModel {
                schema_version: SCHEMA_VERSION,
                name: format!("ellipsoid-{i}"),
                variables: (0..n)
                    .map(|j| Variable {
                        name: format!("x{j}"),
                        lower: Some(center[j] - 3.0 * radius * axes[j]),
                        upper: Some(center[j] + 3.0 * radius * axes[j]),
                        kind: VariableKind::Continuous,
                    })
                    .collect(),
                objective: Objective {
                    direction: if sign > 0.0 { Direction::Minimize } else { Direction::Maximize },
                    offset,
                    linear: c.iter().enumerate().map(|(j, v)| LinearTerm { variable: j, coefficient: sign * v }).collect(),
                    quadratic: vec![],
                },
                constraints: vec![],
            },
            second_order_cones: vec![SecondOrderConeConstraint {
                name: "affine ellipsoid".into(),
                components: (0..n)
                    .map(|j| AffineExpression { offset: -center[j] / axes[j], terms: vec![LinearTerm { variable: j, coefficient: 1.0 / axes[j] }] })
                    .collect(),
                upper: AffineExpression { offset: radius, terms: vec![] },
            }],
        };
        let check = solve_socp_owned(&model, &SolveOptions::default(), Arc::new(NoControl))
            .map_err(|e| e.to_string())
            .and_then(|r| check_optimum(&model.base, &r.result, expected, Some(&point)));
        if let Err(e) = check {
            failures.push(format!("SOCP={i}: {e}"));
        }
    }
    finish("SOCP support geometry", 128, failures);
}

#[test]
fn rank_deficient_hessian_and_redundant_equalities_matrix() {
    let mut failures = Vec::new();
    for i in 0_usize..128 {
        let s = (i % 13) as f64 / 2.0 - 3.0;
        let d = (i % 11) as f64 / 2.0 - 2.5;
        let q = [0.25, 1.0, 4.0][i % 3];
        let offset = (i % 7) as f64 / 4.0;
        let model = OptimizationModel {
            schema_version: SCHEMA_VERSION,
            name: format!("rank-one-QP-{i}"),
            variables: (0..2)
                .map(|j| Variable { name: format!("x{j}"), lower: Some(-8.0), upper: Some(8.0), kind: VariableKind::Continuous })
                .collect(),
            objective: Objective {
                direction: Direction::Minimize,
                offset: offset + q * d * d,
                linear: terms([-2.0 * q * d, 2.0 * q * d], false, false),
                quadratic: vec![
                    QuadraticTerm { left: 0, right: 0, coefficient: q },
                    QuadraticTerm { left: 1, right: 1, coefficient: q },
                    QuadraticTerm { left: 0, right: 1, coefficient: -2.0 * q },
                ],
            },
            constraints: vec![
                LinearConstraint { name: "sum".into(), terms: terms([1.0, 1.0], false, false), relation: Relation::Equal, rhs: s },
                LinearConstraint { name: "redundant sum".into(), terms: terms([-2.0, -2.0], false, false), relation: Relation::Equal, rhs: -2.0 * s },
            ],
        };
        let point = [(s + d) / 2.0, (s - d) / 2.0];
        for sparse in [false, true] {
            let result = if sparse {
                solve_sparse_owned(&model, &SolveOptions::default(), Arc::new(NoControl)).map(|r| r.result)
            } else {
                solve(&model, &SolveOptions::default(), &NoControl)
            };
            let check = result.map_err(|e| e.to_string()).and_then(|r| check_optimum(&model, &r, offset, Some(&point)));
            if let Err(e) = check {
                failures.push(format!("rank={i}, sparse={sparse}: {e}"));
            }
        }
    }
    finish("rank-deficient QP", 256, failures);
}
#[test]
fn explicit_unbounded_and_infeasible_status_matrix() {
    let mut failures = Vec::new();
    for i in 0_usize..128 {
        let negative = i.is_multiple_of(2);
        let sign = if negative { -1.0 } else { 1.0 };
        let a = (i % 17) as f64 / 2.0;
        let b = (i % 13) as f64 / 2.0;
        let scale = [0.125, 1.0, 16.0][i % 3];
        let mut model = OptimizationModel {
            schema_version: SCHEMA_VERSION,
            name: format!("recession-{i}"),
            variables: [a, b]
                .into_iter()
                .enumerate()
                .map(|(j, v)| Variable {
                    name: format!("x{j}"),
                    kind: VariableKind::Continuous,
                    lower: if negative { None } else { Some(v) },
                    upper: if negative { Some(v) } else { None },
                })
                .collect(),
            objective: Objective {
                direction: Direction::Maximize,
                offset: (i % 11) as f64,
                linear: terms([sign * (1.0 + (i % 7) as f64), sign * (1.0 + (i % 5) as f64)], i.is_multiple_of(3), false),
                quadratic: vec![],
            },
            constraints: vec![LinearConstraint {
                name: "linked ray".into(),
                terms: terms([scale, -scale], false, false),
                relation: Relation::Equal,
                rhs: scale * (a - b),
            }],
        };
        let result = solve(&model, &SolveOptions::default(), &NoControl).unwrap();
        if result.termination != Termination::Completed
            || result.feasibility != Feasibility::Feasible
            || result.optimality != Optimality::Unbounded
            || result.best_bound.is_some()
            || result.relative_gap.is_some()
        {
            failures.push(format!("unbounded={i}: {result:?}"));
        }
        model.constraints.push(LinearConstraint {
            name: "contradict linked ray".into(),
            terms: terms([scale, -scale], false, false),
            relation: Relation::Equal,
            rhs: scale * (a - b + 1.0),
        });
        let result = solve(&model, &SolveOptions::default(), &NoControl).unwrap();
        if result.termination != Termination::Completed
            || result.feasibility != Feasibility::Infeasible
            || result.values.is_some()
            || result.objective.is_some()
        {
            failures.push(format!("infeasible={i}: {result:?}"));
        }
    }
    finish("explicit infeasible and unbounded", 256, failures);
}

#[test]
fn four_binary_variable_coupled_rows_match_complete_enumeration() {
    let mut failures = Vec::new();
    for seed in 0..256 {
        let mut rng = Rng(seed ^ 0xd1b54a32d192ed03);
        let costs = (0..4).map(|_| rng.signed(7)).collect::<Vec<_>>();
        let weights = (0..2).map(|_| (0..4).map(|_| rng.signed(4)).collect::<Vec<_>>()).collect::<Vec<_>>();
        let rhs = [rng.signed(5), rng.signed(5)];
        let offset = rng.signed(50) as f64 / 4.0;
        let direction = if seed.is_multiple_of(2) { Direction::Minimize } else { Direction::Maximize };
        let model = OptimizationModel {
            schema_version: SCHEMA_VERSION,
            name: format!("four-binary-{seed}"),
            variables: (0..4).map(|i| Variable { name: format!("x{i}"), lower: None, upper: None, kind: VariableKind::Binary }).collect(),
            objective: Objective {
                direction,
                offset,
                linear: costs.iter().enumerate().map(|(i, c)| LinearTerm { variable: i, coefficient: *c as f64 / 4.0 }).collect(),
                quadratic: vec![],
            },
            constraints: weights
                .iter()
                .zip(rhs)
                .enumerate()
                .map(|(j, (weights, rhs))| LinearConstraint {
                    name: format!("row-{j}"),
                    terms: weights.iter().enumerate().map(|(i, c)| LinearTerm { variable: i, coefficient: *c as f64 }).collect(),
                    relation: Relation::LessEqual,
                    rhs: rhs as f64,
                })
                .collect(),
        };
        let mut optimum: Option<i64> = None;
        for mask in 0..16 {
            let x = (0..4).map(|j| (mask >> j) & 1).collect::<Vec<i64>>();
            if weights.iter().zip(rhs).any(|(w, b)| w.iter().zip(&x).map(|(a, x)| a * x).sum::<i64>() > b) {
                continue;
            }
            let value = costs.iter().zip(x).map(|(a, b)| a * b).sum::<i64>();
            if optimum.is_none_or(|old| if direction == Direction::Minimize { value < old } else { value > old }) {
                optimum = Some(value);
            }
        }
        let result = solve(&model, &SolveOptions::default(), &NoControl).unwrap();
        if let Some(value) = optimum {
            if let Err(e) = check_optimum(&model, &result, offset + value as f64 / 4.0, None) {
                failures.push(format!("binary={seed}: {e}"));
            }
            if let Some(values) = &result.values
                && values.iter().any(|v| *v != 0.0 && *v != 1.0)
            {
                failures.push(format!("binary={seed}: fractional {values:?}"));
            }
        } else if result.termination != Termination::Completed || result.feasibility != Feasibility::Infeasible {
            failures.push(format!("binary={seed}: expected infeasible: {result:?}"));
        }
    }
    finish("four-variable exhaustive binary", 256, failures);
}

#[test]
fn separable_quadratic_open_fixed_and_clipped_bounds_matrix() {
    let mut failures = Vec::new();
    for i in 0_usize..256 {
        let n = 1 + i % 4;
        let sign = if i.is_multiple_of(2) { 1.0 } else { -1.0 };
        let offset = (i % 17) as f64 / 4.0;
        let mut point = Vec::new();
        let mut expected = offset;
        let mut variables = Vec::new();
        let mut linear = Vec::new();
        let mut quadratic = Vec::new();
        for j in 0..n {
            let a = ((i + 3 * j) % 23) as f64 / 4.0 - 2.75;
            let q = [0.25, 1.0, 4.0][(i + j) % 3];
            let (lower, upper) = match (i + j) % 5 {
                0 => (None, None),
                1 => (Some(a + 0.5), None),
                2 => (None, Some(a - 0.5)),
                3 => (Some(a + 0.25), Some(a + 0.25)),
                _ => (Some(a - 1.0), Some(a + 1.0)),
            };
            let x = lower.map_or(a, |lo| a.max(lo));
            let x = upper.map_or(x, |hi| x.min(hi));
            point.push(x);
            expected += sign * q * (x - a).powi(2);
            variables.push(Variable { name: format!("x{j}"), kind: VariableKind::Continuous, lower, upper });
            linear.push(LinearTerm { variable: j, coefficient: -2.0 * sign * q * a });
            quadratic.push(QuadraticTerm { left: j, right: j, coefficient: sign * q });
        }
        let constant = point
            .iter()
            .enumerate()
            .map(|(j, _)| {
                let a = ((i + 3 * j) % 23) as f64 / 4.0 - 2.75;
                sign * [0.25, 1.0, 4.0][(i + j) % 3] * a * a
            })
            .sum::<f64>();
        let model = OptimizationModel {
            schema_version: SCHEMA_VERSION,
            name: format!("separable-{i}"),
            variables,
            objective: Objective {
                direction: if sign > 0.0 { Direction::Minimize } else { Direction::Maximize },
                offset: offset + constant,
                linear,
                quadratic,
            },
            constraints: vec![],
        };
        let result = solve(&model, &SolveOptions::default(), &NoControl).unwrap();
        if let Err(e) = check_optimum(&model, &result, expected, Some(&point)) {
            failures.push(format!("separable={i}: {e}"));
        }
    }
    finish("separable open/fixed/clipped QP", 256, failures);
}
