# Nonlinear, restart and interval optimization

Updated 2026-10-31. This document describes implemented Rust SDK behavior. It does not establish full Frontline parity or Gridcraft host/UI execution.

## Public interfaces

The compiler exports:

```rust
validate_expression_model(&ExpressionModel, &SolveOptions) -> Result<()>
validate_expressions(&[Expr], variable_count: usize, &SolveOptions) -> Result<()>
with_target(&ExpressionModel, target: f64) -> Result<ExpressionModel>
with_target_with_options(&ExpressionModel, target: f64, &SolveOptions)
    -> Result<ExpressionModel>
preflight_target(&ExpressionModel, target: f64, &SolveOptions) -> Result<()>
preflight_target_named(&ExpressionModel, target: f64, row_name: &str, &SolveOptions)
    -> Result<()>
curvature(&Expr, variables: usize) -> Result<Curvature>
evaluate_with_gradient(&Expr, values: &[f64]) -> Result<(f64, Vec<f64>)>
finite_difference_gradient(values, lower, upper, relative_step, evaluator)
    -> Result<FiniteDifferenceResult>
```

`validate_expression_model` validates metadata, options, variable domains and handles, constants, depth, one shared node budget across the objective and all rows, and a checked memory estimate. It does not evaluate a point or require affine/quadratic structure. `with_target` retains the objective and direction and adds an equality between the original objective and the target. It does not minimize a squared target residual as a substitute for that equality.

`validate_expressions` applies the same shared node/memory contract to borrowed
outputs before an API builds or clones a temporary model. Target callers with
configured budgets use `with_target_with_options`; the legacy `with_target`
delegates to default options. The options-aware helper checks the expanded
objective copy, target constant, new row and all metadata before cloning any
model fields. Hosts can use `preflight_target_named` and append the same exact
equality in place, preserving an existing row label. Four target tests cover
original-unit equality, an original model that fits while its expansion exceeds
the caller budget, exact/one-byte-below boundaries, the shared duplicated-node
limit and the actual host row-name cost. These estimates are accounting bounds,
not measured process RSS.

The facade exports:

```rust
nonlinear::solve(model, options, control)
nonlinear::solve_from(model, start, options, control)
nonlinear::solve_from_with_derivatives(model, start, options, control, mode, step)
constrained::solve(model, options, nlp_options, control)
constrained::solve_from(model, start, options, nlp_options, control)
multistart::solve(model, options, nlp_options, multistart_options, control)
multistart::solve_from(model, start, options, nlp_options, multistart_options, control)
global::solve_box_polynomial(model, options, control)
global::verify_box_certificate(model, report, options)
```

`NlpOptions` holds outer and inner iteration limits, initial penalty, penalty growth, derivative mode and finite-difference step. `MultiStartOptions` holds the number of starts and a full-u64 seed serialized as a decimal string. These are local module contracts; existing `SolveOptions` and objective direction DTOs are unchanged.

## Derivatives and curvature

Reverse AD differentiates the bounded operator AST. A derivative failure is an error, never an invented zero. Raw `abs(0)`, tied min/max, and an active square root at zero are not smooth. The composite `max(0,g)^2` has a dedicated chain rule: its derivative is zero at `g=0`, where the composite is continuously differentiable. This rule does not make raw maximum ties differentiable.

Explicit finite-difference mode runs an actual evaluator for each probe. Unequal central steps adapt to bounds. One available side uses a second-order one-sided estimate where a second point is representable. Steps are scaled by coordinate size and capped relative to finite interval width. Fixed variables have zero feasible sensitivity. Unrepresentable perturbations, two failed sides, nonfinite values and exhausted budgets remain explicit errors or termination reasons. Domain rejection on one side can use the valid side. Every successful/failed evaluator callback consumes the same evaluation budget. Automatic mode uses AD first and permits numerical fallback only for an explicitly unavailable derivative; a nonsmooth or invalid-domain error is not swallowed.

Finite differences are approximations. Their stationarity residual is labeled approximate; they provide no optimality certificate. The standalone finite-difference function accepts a Rust callback, allowing black-box evaluators without an AST derivative. The current expression AST already has AD implementations for its supported smooth operators.

`Curvature::{Affine,Convex,Concave,Unknown}` follows sufficient composition rules, literal constant scaling, affine even powers, exp of convex expressions, log/sqrt of concave expressions, and supported extrema. No sample values prove convexity. Unrecognized products and algebraically equivalent forms may remain Unknown. Domain validity remains a separate obligation. Engine-owned Hessian analysis handles additional quadratic proofs; this AST classifier is not a replacement for a numerical optimality certificate.

## Bound-constrained local search

The local bound method is scaled projected gradient with Armijo backtracking and Barzilai-Borwein step estimates. It is not GRG. It accepts continuous variables, bounds and smooth objectives. Starts are projected into bounds with a visible warning. The projected gradient suppresses only an outward derivative at an exactly active bound, or a fixed variable; a small displacement is not stationarity.

The strongest successful local status is `Stationary`. No local result has a global best bound. Finite objective magnitude, including a bounded `-1e16*x`, never implies unboundedness. Initial domain/derivative failure is explicit, and invalid line-search candidates do not overwrite a previously valid point.

## Smooth general constraints

The constrained method is a safeguarded augmented Lagrangian with the bound method as its inner solver. Equalities are normalized as `h=left-right=0`; inequalities as `g<=0`, reversing GE rows. The minimized objective is signed for minimization/maximization. The merit terms are:

```text
equality:   lambda*h + rho/2*h^2
inequality: max(0, mu+rho*g)^2/(2*rho)
```

The omitted inequality constant is independent of the decision variables. Equality multipliers are updated by `lambda += rho*h`; inequality multipliers by `mu=max(0,mu+rho*g)`. Multipliers and the penalty have finite safeguards. Penalty growth is conditional on insufficient constraint improvement. The sum tree is balanced and merit expansion is capped before cloning.

After each inner solve, the original objective, rows and derivatives are evaluated independently. `Stationary` requires original-unit primal feasibility, a bound-projected Lagrangian-gradient infinity norm within the requested optimality tolerance, and complementarity within the stated tolerance. Interrupted searches retain the best previously verified feasible point. An infeasible local endpoint has feasibility Unknown, not a proof of infeasibility. Integer variables remain unsupported. Dense Jacobian storage limits scale; this is not a sparse industrial NLP implementation. Poor conditioning, degenerate constraints, invalid initial domains and finite safeguards can prevent convergence. The finite iteration implementation does not inherit an unconditional convergence theorem merely from its algorithm family.

## Configured multistart

The caller's start is run first; further starts are deterministic, counter-addressed uniform samples of the finite box. More than one start requires finite bounds. The remaining evaluation and iteration allowances are divided across remaining runs, so budgets are shared rather than reset. A failed start returning an error cannot return its private counters; its complete allocated allowance is conservatively charged and this reservation is disclosed in warnings. No optimality bound or global guarantee is inferred from repeated local stationarity. Feasible candidates are compared in the original objective direction; otherwise the smaller validated violation is retained.

## Interval global certificates

The certified subset is a continuous finite box, at most 64 variables, no general constraints, and a polynomial AST using arithmetic, nonnegative integer powers up to 64, and division by a nonzero literal constant. Transcendental functions, general rational expressions, integer domains and general nonlinear constraints are explicit unsupported cases.

Natural interval evaluation rounds every elementary endpoint operation outward using adjacent f64 values. Integer powers use repeated interval multiplication, rather than assuming a platform `powf` is correctly rounded. A feasible midpoint's point enclosure supplies the incumbent upper bound. The smallest unsearched box lower bound supplies the global lower bound on the signed minimization objective. For maximization the sign is reversed when exposing `best_bound`. The requested absolute or relative objective gap must be satisfied before `Optimal` is returned. Overflow and unrepresentable gaps are numeric errors.

Node, evaluation, iteration, memory and cancellation stops retain the parent's region until both children can be constructed; they cannot accidentally discard an unsearched bound. Split witnesses record a partition of the entire input domain, including regions pruned from the active queue. `verify_box_certificate` independently replays the partition, recomputes all leaf enclosures and the incumbent enclosure, checks the original domain and point, and rejects altered bounds, missing parents, invalid splits and changed values. It uses the same specified elementary interval primitives but does not trust the solver's queue or stored lower bounds. This is an independently replayable numerical certificate, not a theorem-prover proof or a general MINLP certificate.

## Requirement evidence

| Requirement | Implementation and direct regression evidence | Remaining scope |
|---|---|---|
| R001 Zieldefinition | `target::tests::target_preserves_objective_and_enforces_original_units`; `constrained::tests::target_goal_preserves_original_objective_value`; existing bound min/max tests | Host dialogs and SDK workflow dispatch are separate integration work. |
| R008 Konvexität | `curvature::tests::*`; structural disguised-nonlinearity tests; engine Hessian analysis is separate | Sufficient rules intentionally leave some convex forms Unknown. |
| R011 Differenzenrückfall | `finite_difference::tests::*`; `nonlinear::tests::finite_difference_mode_counts_all_probes_and_respects_tiny_bounds`; constrained explicit-FD test | Approximate derivatives can be inaccurate on ill-conditioned or nonsmooth functions; no invented exact certificate. |
| R017 Glatte NLP | Analytic equality `x+y=1`; LE/GE active inequalities; nonlinear `x^2<=1`; inactive rows; infeasible-search Unknown; evaluation budget; original-unit default tolerance test | Dense continuous local method; no sparse KKT solve or general infeasibility/global proof. |
| R020 Mehrfachstart | Reproducible escape from an independently computed worse stationary basin; shared-budget test; finite-bound/input validation; lossless seed JSON | Uniform box starts only; no basin enumeration guarantee. |
| R079 Globalverfahren | Quartic with multiple minima; maximize signs; interrupted bounds; dense independent grid enclosure; replay and tampering tests | Certified continuous polynomial boxes only; no general constrained transcendental/MINLP proof. Local methods never export a global bound. |

Targeted execution uses real workspace crates, not stubs. Run `cargo test -p solvercraft -p solvercraft-compile --offline --locked`, plus required workspace fmt/clippy/test gates. Full Gridcraft compilation and UI execution are additional acceptance gates and are not implied by SDK tests.

## Primary algorithm references

- E. G. Birgin and J. M. Martínez, [Practical Augmented Lagrangian Methods for Constrained Optimization, Chapter 4](https://epubs.siam.org/doi/10.1137/1.9781611973365.ch4), SIAM, 2014; [authors' book page](https://www.ime.unicamp.br/~martinez/book.html).
- T. Csendes and D. Ratz, [Subdivision Direction Selection in Interval Methods for Global Optimization](https://epubs.siam.org/doi/10.1137/S0036142995281528), SIAM Journal on Numerical Analysis, 1997.
- [Hybridizing two linear relaxation techniques in interval-based solvers](https://link.springer.com/article/10.1007/s10898-024-01449-2), Journal of Global Optimization, 2024, for the interval branch-and-bound framework and treatment of floating-point rounding. This implementation does not implement that paper's linear relaxations.

The code is a clean-room implementation of public mathematical methods. No proprietary Frontline code or undocumented method is represented as copied or equivalent.
