# Ellipsoid formation precision review

Executed on Windows, Rust 1.95, using the locked SDK workspace and isolated engines target. This note covers R070 coefficient formation and original-domain validation, not commercial robust-optimization parity.

## Reproduced defect

Let `e=2^-52`, `radius=1-e`, `factor=[[1+e,1,1]]`, and fix the three variables to `[2^104,-(2^104+2^52),-1]`. The nominal constraint has no terms and RHS zero. Exact original accumulation gives `radius*L*x=-(1-e)`, hence robust support almost one and a material violation.

Before production edits, the executed Rust test `ellipsoid_coefficient_formation_never_accepts_a_material_original_violation` failed: `validate_conic_solution` accepted the rounded counterpart with reported violation `2.220446049250313e-16`. Rounding `radius*(1+e)` to one erased the original cancellation residual. The CLI owner separately executed the old command and observed Unknown without a point or bound; no false-feasible CLI result was established.

## Corrected contract

`ellipsoid::compile` retains the original variable shape and Lorentz-cone representation. For each formed coefficient `c=round(radius*L_ij)`, it accumulates the exact residual `radius*L_ij-c`. With finite original bounds, the sum of absolute residuals times `max(|lower_j|,|upper_j|)` gives a uniform L1 bound `E` on the component perturbation, which also bounds its Euclidean norm. Exactly representable coefficients need no finite-box assumption.

Material `E`, unavailable required bounds, or unsupported finite precision return `numeric_range`. Small accepted `E` is added to the cone RHS with directed outward rounding. The additional RHS rounding is also bounded. Thus every original feasible point remains feasible in the counterpart: by the triangle inequality, `||rounded(radius L)x|| <= ||radius Lx||+E`. The counterpart's verified optimization bound remains a bound for the original feasible set.

`ellipsoid::validate_original` independently accumulates the original `radius*factor*x` triples exactly before rounding. Scaled, directed sum-of-squares enclosures bound the norm without squared-product overflow; original nominal rows and the objective use shared exact arithmetic. This can conservatively refuse numerically ambiguous boundaries rather than report unsupported feasibility.

`ellipsoid::solve_owned` compiles once, calls the conic backend, and checks every returned incumbent against the original rows. Its serializable `{counterpart,result}` layout preserves the previous CLI result shape. Original violations return a structured precision error. Compilation and backend factorization remain bounded setup operations rather than hard realtime preemptible code.

## Executed evidence

The unchanged `tests/ellipsoid.rs` passed **3/3**. New `tests/ellipsoid_precision.rs` passed **3/3**: the material-cancellation guard, an ordinary decimal coefficient/formation allowance with checked optimum and valid upper bound, and a two-component circle with independent feasible/infeasible point checks and the analytic optimum `x=0.8`.

Selected facade Clippy with `-D warnings`, owned-file rustfmt, and facade WASM compilation passed. The CLI owner independently executed the material-input `numeric_range` error with its manifest, the identity-circle control, and an ordinary decimal control. These finite regressions and conservative arithmetic guards do not prove unrestricted numerical stability or unrestricted sparse scale.
