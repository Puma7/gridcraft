# Solvercraft optimization engines

Workbook-independent Rust optimization with explicit numerical and resource limits.

| API | Implemented algorithm |
|---|---|
| `solve` | Independent two-phase dense simplex, linear integer branch-and-bound, analytic separable quadratic fast path, general continuous convex QP by proximal ADMM |
| `solve_qp_detailed` | Checked general linearly constrained convex QP, primal/dual warm starts and independently restored Lagrangian bounds |
| `solve_sparse_owned` | Actual sparse CSC LP/QP via pinned Clarabel 0.11.1 and sparse QDLDL/AMD |
| `solve_socp_owned` | Exact affine Lorentz cones, continuous LP/QP/SOCP interior point, cone duals and original-domain validation |
| `solve_mixed_conic_owned` | Finite integer-domain MIQP/MISOCP branch-and-bound with verified incumbents and valid relaxation bounds |
| `compile_discrete`, `solve_discrete_owned` | Bounded SOS1/SOS2 selector formulations and indicators with finite-bound-derived big-M; independent original semantics check |
| `solve_lp_with_basis` | Checked original-coordinate vertex basis reuse, shadow prices, and local allowable RHS/objective changes |

The reference simplex uses a budget-capped dense tableau. The explicit sparse path really uses sparse CSC matrices and sparse factors, with a conservative worst-case fill memory estimate. Checked dense QP factorization is limited to 512 variables; vertex basis/sensitivity to 64 variables and 512 normalized rows, with bounded basis search. These limits are explicit rather than claims of unrestricted commercial-suite parity.

Every returned candidate is validated against the original model; integer candidates are rounded and validated again. A backend status, small gradient or large objective magnitude alone never establishes a global optimum, infeasibility or unboundedness. Missing certified bounds stay absent. Approximate infeasibility rays must independently separate the original variable box. A supported unbounded result requires a feasible anchor and a checked recession direction.

Cancel, iteration, evaluation, node and memory limits remain distinct from infeasibility. The conic primary APIs take `Arc<dyn RunControl>` for safe synchronous callbacks on native and WASM. Native borrowed compatibility uses a scoped worker bridge; WASM callers use the owned API. Clarabel does not expose a public warm-start point API; no such capability is advertised. LP vertex-basis and independent QP primal/dual warm starts are separate, real implementations.

Arithmetic uses f64 and declared original-unit tolerances. No exact rational certificate, arbitrary-scale correctness, complete PSD recognition, cutting-plane MIP suite, sparse basis-factor reuse, or browser runtime validation is claimed. Unsupported classes return structured errors or conservative Unknown/Stationary results.

See [implementation evidence and remaining gaps](../../docs/implementation/engines.md) for algorithms, dependency licensing, tests and restrictions. From the standalone workspace run `cargo test -p solvercraft-engines --offline --locked`, `cargo clippy -p solvercraft-engines --all-targets --offline --locked -- -D warnings`, and `cargo check -p solvercraft-engines --target wasm32-unknown-unknown --offline --locked`.
