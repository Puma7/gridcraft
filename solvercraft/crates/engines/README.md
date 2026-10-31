# Solvercraft reference engines

This crate implements real, workbook-independent optimization. It is a bounded alpha backend, not a replacement for the full capability set of HiGHS, Clarabel or Frontline.

## Implemented engines

| Engine | Supported model | Result guarantee |
|---|---|---|
| solvercraft-dense-simplex-0.1 | Linear objective, linear <=/=/>= rows, free/fixed/one-sided/two-sided continuous variables | Floating-point two-phase simplex, subject to configured tolerances and original-model validation |
| Same backend with branch and bound | Linear model with integer/binary variables | LP-relaxation bounds, verified incumbent, conservative termination and global-gap reporting |
| solvercraft-separable-convex-qp-0.1 | Continuous variables, bounds only, convex separable quadratic minimization or concave separable quadratic maximization | Analytic coordinate minima; no finite-difference gradient approximation |

The model stores sparse coefficient lists. The LP reference backend deliberately uses a dense tableau internally. A conservative tableau/working-set estimate is checked against max_memory_bytes before construction. This is not a large-scale sparse factorization engine.

Auto routes LP/MILP or the supported separable QP class. Simplex on an integer model is rejected rather than silently relaxing integrality. ProjectedGradient and DifferentialEvolution are not implemented by this crate and return an explicit error. General constrained/cross-term QP, SOCP, general NLP, MIQP and global nonlinear optimization are not advertised.

## Numerical and interruption semantics

- No magnitude threshold is interpreted as mathematical unboundedness.
- A very small positive entering-column coefficient prevents an unreliable unbounded-ray classification; the routine reports numerical failure instead.
- LP-unbounded does not establish MILP-unbounded. When this reference search encounters such a relaxation it returns an honest unknown mixed-integer status.
- Every returned candidate is checked against the untransformed model. Integer candidates are rounded and then validated; rounded feasibility is never assumed.
- Cancel, iteration, evaluation, node and memory budgets remain distinct from infeasibility. Verified integer incumbents survive interruptions.
- The global lower bound in minimization space comes from unresolved nodes' valid LP bounds. It is omitted if an unresolved node has no available bound.
- Relative gap is max(incumbent-bound,0)/max(1,abs(incumbent),abs(bound)); maximization is transformed consistently.
- Positive requested MIP gap termination is BestFound with an explanatory warning, not an exact zero-gap proof.
- No primal/dual basis, shadow prices, sensitivity intervals, warm start or exact rational certificate is exposed.

The arithmetic is f64 with absolute original-model feasibility tolerances. Poorly conditioned problems can return numerical failure. No exact arithmetic, arbitrary-scale correctness or performance parity is claimed.

## Verification

The tests cover Wyndor's LP objective 36, alternative optima, equality/free/fixed/negative variables, binary optimum 1 versus continuous relaxation 1.5, infeasibility, actual LP-unboundedness, the LP-unbounded/MIP-infeasible trap, retained incumbents and bounds on interruption, large finite objective values, narrow-bound analytic QP, tiny-pivot honesty and independently enumerated signed integer models.

Run cargo test -p solvercraft-engines --offline and cargo clippy -p solvercraft-engines --all-targets --offline -- -D warnings from the standalone Solvercraft workspace. A native core test is not the full Gridcraft or WASM runtime gate.

