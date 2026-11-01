# Solvercraft runtime and guarantee review

Review date: 2026-10-31.

The review started from PR predecessor **690a098a6c4c9d966d66b6c30c036e0e42534bfd**, based on Gridcraft **981fad14e0b90a7906a02f29785a6d0eb214fee6**. The local checkout is a materialized source tree without Git metadata. The source identities below identify the reviewed local fixes; the integrator associates the final commit with the consolidated verification report.

Scope: facade risk.rs, jobs.rs, provenance.rs, seed codecs, advertised README guarantees and the 96 masterplan requirements. This was functional/numerical QA, not a security certification or a claim of complete solver parity.

## Accepted final5 verification

The completed source-bound entry is [final-verification-summary.json](../implementation/final-verification-summary.json). The final host `cargo xtask ci` accepted all ten stages on unchanged source: **2827 passed, 0 failed, 1 pre-existing ignored**, in 722.25 seconds. The ignored `recalc_with_range_formulas_scales_linearly` is explicitly retained and is not counted as passed. The separate SDK accepted formatting, all-target Clippy with warnings denied, and **630 tests passed, 0 failed, 0 ignored**. The scoped suites described below are not added to those totals again.

Actual final5 Chrome execution passed **13 SDK tests and 7 production-worker tests**. Native UI passed **6**; the chart module passed **8 numerical contracts plus 1 retained-result lifecycle case**, rather than nine numerical tests. The final worker observed maximum active progress/status/heartbeat gaps of **65/1/22 ms**. Release oracles passed **30 LP, 20 QP, 20 ordinary robust + 4 tiny signed cases, 3 exact binary variants and 1 node-limit control**. Five fresh release processes per performance workload retain hardware, seed, memory, time and numerical-error evidence with shared-host activity disclosed.

The code/compilation-input raw SHA-256 is `fcc9d93350d3c5d3342b02c775096867d01c0e7addfdc9099ac0160cb460ecfe`; canonical SHA-256 is `9188b27559e9f23fabe2faa8c353c2e5fdca1bc6ab113955375ccd46111e88bb`. The release CLI SHA-256 is `68a6ab8da2646ddb82ca312b6c8977e8c3cf596ed802b8f343e92d51d662c6e1`. These source/binary identities differ in purpose from historical diagnostic fingerprints. The original 96-criterion assessment now records **94 bounded-accepted and 2 partial (R046/R081)**. All original wording remains intact, and full Frontline/PSI parity is not claimed.

## Subsequent implementation and independent engine review

The 265-test report and source hashes below are a **historical frozen baseline**. They do not describe the newly added optimization/risk/workflow modules. See [stochastic implementation evidence](../implementation/stochastic.md) for the new model classes and limitations.

A later native standalone workspace run passed **389 tests, zero failures/ignored**, on the then-current source using Rust 1.91.1. Its local log is `$BUILD_OUTPUT\reference-review-workspace.log`. The declared workspace toolchain was subsequently aligned to the actual parent-supported **Rust 1.95.0**; older-toolchain evidence is not presented as that new toolchain's release gate.

Independent review on exact Rust 1.95.0 added `crates/engines/tests/certificate_review.rs`. Nine tests pass, with analytic circle min/max bounds, exhaustive binary MIQP/MISOCP optima and bounds under interruption, LP RHS/objective basis changes, forged bases, finite tiny blockers, cancellation and exact dyadic accumulation. Internal enumerated cases are not added to Cargo's test count. The following counterexamples were executed before their owner corrections:

| Confirmed defect | Counterexample and correction evidence |
|---|---|
| Global proof midpoint outside a fixed domain | `x=smallest positive subnormal`, objective `1e308*x`: midpoint rounded to zero, yet Optimal/zero-gap was reported. Owner preserves/clamps fixed-box midpoints; `tests/global_review.rs` now passes this case, normal/minimum-normal controls and certificate replay. |
| Warm basis bypassed method validation | A valid warm LP basis returned Optimal for an explicit ProjectedGradient/DE request while the cold path rejected it. Owner now rejects those methods before the warm path; independent regression passes. |
| Public conic validation ignored its work budget | A zero-variable model with 20,000 zero cone components returned feasible with a one-byte budget, after allocating a component scratch vector. Owner added dimension/budget preflight and streamed norm evaluation; independent regression passes. |
| Big-M transformation removed feasible points | `b=1 => x<=3`, `0<=x<=1e20`: `M+3` rounded to `M`, so the transformed active row excluded `[3,1]`. Both engine and facade now use a separate bounded slack and exact gate, keeping the original RHS separate from M. Facade also derives M with outward products/additions and preflights every new variable/row/term; chance/equality/reification dimensions were updated. Original active-point controls with zero auxiliary slack pass. |
| Basis precision proof ignored multiplication residuals | Fixed `x=1+2^-52,y=1`, cost `(2^53-1)*x-2^53*y`: true dyadic optimum `1-2^-52`, but warm basis reported Optimal objective 0/gap `1.42e-13`. The owner first added a truthful `numeric_range` refusal, then implemented a central 104-limb exact dyadic sum/product/triple accumulator and original-unit certificate arithmetic. Independent regressions verify this objective, carries, subnormal ties, collective product underflow, maximum-f64 triple cancellation and exact-sign finite blockers. |
| Near-integral LP point falsely closed an integer node | Binary `x,y`, maximize `1e12*x+500*y`, `1e9*x+y<=1`: the old actual CLI returned Optimal/0 after rounding the LP point `[1e-9,0]`. Independent four-point integer enumeration gives 500 at `[0,1]`. The corrected search compares incumbent/relaxation objectives without the additive constant and branches on an actual nonzero integer fraction while a material gap remains. Exact min/max, offset and interrupted-bound controls now pass. |

The extended root-owned workflow manifest now records actual engine, source/snapshot/run context and goal/nonlinear configuration, rather than relying on the legacy manifest alone. Independent review exposed another configuration defect: the bound-only nonlinear path ignored a requested finite-difference mode and used AD. The workflow owner corrected it. Three tests in `crates/solvercraft/tests/workflow_review.rs` pass on exact Rust 1.95.0: explicit/Auto FD selection is honored; goal, nonlinear mode/step, actual engine, large revisions and maximum-u64 seed survive manifest serialization; and representative binary64 values round-trip without transport rounding. Original objectives are verified independently.

Two later owned-model regressions were also reproduced and corrected. Ordinary decimal mean-variance coefficients had been emitted as duplicate generated terms, conflicting with the strict raw-input exact-coefficient policy. The risk compiler now emits each combined coefficient once, records a conservative bound on **final coefficient assembly rounding over the original finite variable box**, reevaluates original scenario risk and weakens global bounds/status when that allowance matters. Earlier moment-computation conditioning checks remain engineering guards; this field is not an interval proof for all moment arithmetic. Unbounded coordinates with nonzero unbounded coefficient error are explicitly refused. The decimal analytic optimum and a tighter-than-roundoff optimality request are tested. Separately, priors `[0.5,0.5000000000005]`, constant payoffs `1e17` and a free uninformative signal produced a fictitious value 50000 while perfect information had value zero. Decision analysis now normalizes accepted priors/likelihood columns consistently and uses a weighted outer signal mean. The original counterexample gives exactly zero; linear/CARA uninformative and revealing signals with acquisition costs pass.

## Subsequent independent mathematical and simulation review

The additive exact-1.95 suites `crates/math/tests/numerical_review.rs` and `crates/simulation/tests/numerical_review.rs` each pass nine tests. They cover finite heavy-tail log densities, independent inverse-Gaussian/beta anchors, reflected discrete quantiles, reflected/negative-parameter copula conditionals, extreme beta-prime and skew-normal CDFs, fitting likelihood/AIC/BIC, refitted bootstrap reproducibility, configured-checkpoint stopping, copula moments/quadrants, independent LHS replicate identity and fixed-sample t intervals. Later controls reconstruct a supplied covariance from transformed basis vectors, verify zero/rank-one PSD and latent-lognormal semantics, preserve intact resampled rows/columns, and check whole-array permutations plus SIP/SLURP replay boundaries. These new multivariate/resampling controls are separate from the scalar SciPy grid.

Executed counterexamples led to owner fixes for finite Cauchy/skew-normal log densities incorrectly becoming negative infinity, reflected Bernoulli quantiles, reflected Clayton conditionals, cancellation in beta-prime/skew-normal tails, Student-t upper-tail inversion, mixed PMF/PDF AIC ranking and optional-stopping intervals published after off-checkpoint cancellation. A real duplicate-seed LHS run also exposed that replicate labels alone did not create independent draws; interval validation now requires genuinely distinct supported sampling identities. Fixed-sample t intervals retain their stated Gaussian/independent-replication assumptions, with an explicitly approximate large-df critical-value implementation. These finite tests do not establish universal tail-relative accuracy or distribution-free optional-stopping coverage.

The isolated **test-only** SciPy 1.18.1/NumPy 2.5.4 oracle is reproducible through `tests/e2e/distribution_oracle.py`; these are not Rust/product dependencies. Their project BSD-3-Clause licenses and installed wheel notices remain with the test environment. [distribution-oracle-report.json](distribution-oracle-report.json) records parameter mappings, thresholds, tool versions, controlled binary64 transport checks, harness/source hashes and the before/after source-stability check. On the recorded stable math source, 150 parameterizations produced **1349 quantile, 1361 CDF and 1361 log-density comparisons, zero failures and zero changed transport values**. One inverse-Gaussian quantile was excluded because the independent SciPy/Boost oracle itself emitted a nonconvergence warning; the exclusion is recorded. Maximum observed quantile relative error was `3.771408747484251e-8`, CDF absolute error `6.859614753995658e-8` (within the separately declared legacy normal/lognormal tolerance) and log-density absolute error `1.6198598018490884e-11`. These are finite parameter-grid results, not all-parameter accuracy guarantees. The Sobol-v2 test independently matches all 16,384 coordinates of SciPy's first 256 un-scrambled 64-dimensional points using a precomputed exact-byte fingerprint.

## Subsequent public API review

On exact Rust 1.95, the new facade `tests/api_review.rs` has twelve passing tests in the integrator's latest matched run, and CLI `tests/api_review.rs` passes four real-process tests. They independently verify nested JSON strictness, subnormal/binary64 fidelity and maximum decimal IDs/revisions/seeds, canonical revisions, atomic failed/colliding/exhausted updates, the 128-model portable quota, migration, unique class-specific registry selection, owned integer-QP routing, distinct cancellation/deadline statuses, per-connection authentication, revoked authentication after a wrong replacement token, retained-job quota/forget behavior and bounded response fallback without losing a valid request ID. A real excess TCP connection is refused and its slot becomes usable after the active client disconnects.

Executed counterexamples identified three corrected public-contract defects. An authenticated connection already blocked in a read could submit a new job **after `Server::run` returned from shutdown**; the server owner now checks stopping after read, at dispatch and under the shared admission lock before starting either job kind. Analytic separable QP progress incorrectly named its stage `simplex`; the engine owner now emits its actual stage. Finally, 4096 individually small output ASTs totaling 126976 nodes bypassed the service's intended shared 100000-node limit, while the CLI cloned all ASTs before its global check. A borrowed compiler preflight now enforces one node/depth/memory budget before cloning in the service and all three CLI simulation paths. The same over-budget request is rejected in real TCP and process tests. One-byte process budgets fail before evaluating a deliberately undefined `ln(-1)`. Response byte quotas remain distinct from measured operating-system RSS or preemption guarantees.

A separate source-order review found that the owned integer-QP auto route cloned the model before its memory/validity admission check. The engine owner moved validation before that clone, preserving explicit infeasible-bound semantics. This is source-order evidence, not measured process memory. After these changes, all-target Clippy with `-D warnings` passed for facade, CLI, math and simulation on exact 1.95.

A later actual-wire control found that retained-simulation admission counted sample cells and a constant envelope but omitted dimension-dependent summaries: 1000 constant outputs and one retained trial produced a completed response larger than 100000 bytes, yet the 100000-byte service still admitted it. The failing independent regression was preserved, then passed after the server added a conservative per-output summary bound. Typed response-size preflight now precedes constructing a JSON Value. Separately, options-aware target expansion preflights the caller's memory budget before duplicating the objective; the equality semantics preserve original units. These are specific corrected admission failures, not an operating-system peak-memory theorem.

Run metadata has subsequently expanded beyond the initial provenance baseline. The shared streaming typed helper has three executed tests; eight real CLI process suites cover effective configuration/actual engines/high seeds and enriched legacy solve; five actual server TCP tests cover retained optional manifests; native host tests cover real snapshot identity and simulation rerun configuration. See [run-manifests.md](../implementation/run-manifests.md) and the [96-criterion assessment](acceptance-current.md). The host active-sheet admission race was corrected and verified with a deterministic threaded barrier and nested parameter-job controls; final5 source/platform gates now pass. These additions do not transfer an earlier source hash or build result to a later source state.

The post-admission CLI error path was tested separately: a valid finite five-state information-value problem performs utility work then fails because its information-value spread is not representable. It originally emitted only code/message, discarding prepared identity. The CLI owner corrected all prepared paths to stream one `{code,message,manifest}` error while keeping stdout empty. The independently added regression verifies repeated/changed request fingerprints, platform, entrypoint, diagnostic snapshot and effective configuration. The owner's later 66-process-test gate includes the corrected checked robust routes and integer-node closure controls; no old source hash is reassigned to that run.

## Original robust support correction

An executed counterexample exposed a false Feasible/Optimal box result: fixed `x=1e-9`, nominal empty row `<=0`, radius `1e12`; the old result was `[1e-9,0]` with original worst-case support `1000.0000000000001`, objective/bound zero, and transformed max violation only `1e-9`. The radius-one control did not expose it. The old auxiliary constrained `abs(x)` in small variable units and multiplied its allowed slack by the large radius in the robust row.

Weighted-support auxiliaries now keep the radius inside their defining rows. Checked owned box/budget solve APIs compile once, solve, independently evaluate original box or fractional-cardinality support and original objective, and only then retain feasibility/global claims. Their original validators use exact product accumulation and directed residuals. Exact significand/exponent sort keys preserve product order even when nearest binary64 products tie; full/fractional support survives collective product underflow. A positive invalid original residual produces a numerical error, not a feasible result. Plain counterpart construction remains a mathematical compilation API; users solving it through a custom backend must invoke original validation themselves.

The unchanged counterexample and prior controls pass. The exact-1.95 risk follow-up passed **13 runtime-review and 5 SAA/risk tests**, plus facade all-target Clippy with `-D warnings`. Added controls include summed small auxiliary slacks, a fractional selection where rounded-product ties reverse feasibility, and collective subnormal products. The independent CLI owner additionally passed box/budget infeasible, same-scale feasible and signed-coordinate process controls; the independent original-space uncertainty-vertex oracle is recorded separately. Ellipsoid precision was independently corrected by the engine owner; see [ellipsoid-precision.md](ellipsoid-precision.md).

## Latest integer-node closure correction

The final small MIP probe found an optimality defect that original-primal validation alone could not detect: `[0,0]` was feasible, but not optimal. For binary `x,y` under `1e9*x+y<=1`, only `[0,0]` and `[0,1]` satisfy the exact constraint; their objective values for `1e12*x+500*y` are 0 and 500. The continuous relaxation can choose `[1e-9,0]`, whose value is 1000 even though its first coordinate lies within the integer-distance tolerance. Rounding it to zero therefore yields an incumbent, not an integer optimum certificate.

The executed pre-fix CLI reported Optimal/0. The engine now retains a rounded feasible incumbent but closes its node only when the constant-free incumbent and relaxation bounds justify closure. Exact-sign comparisons prevent a large additive offset from hiding the remaining objective difference. An actual fractional coordinate can still require branching inside the nominal integer-distance tolerance. Mixed-conic unresolved leaves likewise remain open instead of being closed from backend status alone.

The corrected ordinary path returns `[0,1]`, objective/bound 500 and zero gap; the minimization counterpart returns -500. The `1e16` additive-offset process control still finds the exact 500-unit improvement. With one node, the verified zero incumbent remains BestFound with NodeLimit and an open valid bound/nonzero gap. An executed conic probe retained BestFound at 500, a valid upper bound about `500.0000009476` and one unresolved leaf; that numeric bound is a recorded observation, not a promised exact iteration outcome or permission to relabel the result Optimal.

Concrete evidence: [engine integer-rounding controls](../../crates/engines/tests/integer_rounding.rs), [exact CLI enumeration](../../crates/cli/tests/end_to_end.rs), [offset/node-budget process controls](../../crates/cli/tests/manifests.rs), and [engine implementation record](../implementation/engines.md). The engine owner passed **89 engine tests**, strict all-target Clippy and WASM checks; the CLI owner passed **66 process tests**, Clippy and formatting. These are scoped source-specific checks. The refreshed **final5** whole-source/browser proof is accepted, including exact objective-500 variants; earlier final4 checkpoints remain historical rather than latest acceptance.

These are functional/numerical findings. They do not establish full UI/browser/platform acceptance or close all 96 requirements. The remainder of this report intentionally preserves the earlier baseline and its limits.

## Historical 265-test conclusion and release boundary

Two additional confirmed defects were found and fixed during this review:

1. Weighted CVaR could clip the final scenario's probability mass and return the wrong sign at confidence zero.
2. The unit expected_loss risk variant silently accepted additional fields despite a deny-unknown-fields annotation.

The examined finite counterexamples now pass. No unresolved executed defect remains in these reviewed facade paths at the recorded source identities. This is not a proof of absence of other defects.

The integrator's final frozen standalone run recorded **265 tests passed, zero failed, zero ignored**, plus formatting, all-targets Clippy with -D warnings, release build/examples and a Linux cross-target compile check. The current evidence is [verification-report.json](verification-report.json). The original 136-test report remains historical and must not be mistaken for the expanded QA run.

The complete Gridcraft build, **17 host integration tests (4 existing + 13 new)**, visual UI validation and actual WASM/browser execution remain unexecuted. Their source was inspected, not behaviorally proven. Full R1/R2/R3 and Frontline parity remain open; see [coverage-matrix.md](coverage-matrix.md).

## Confirmed defect 1: weighted tail mass rounding

The previous helper repeatedly subtracted normalized probabilities from a scalar remaining tail. With losses [10^17,-1,-6*10^16], raw weights [3,2,5] and confidence 0, it returned **3.8**. Confidence-zero CVaR must equal the weighted mean:

    (3*10^17 - 2 - 5*6*10^16) / 10 = -0.2

After subtracting 0.3 and 0.2, the scalar remaining mass became slightly smaller than 0.5. The helper clipped the final scenario's 0.5 mass; the tiny mass error multiplied a large negative loss. Compensating only the loss sum did not repair the incorrect mass.

Executed controls included [0,10] with weights [3,1], confidence 0.5 (expected 5), and a tiny constant loss 10^-310 near confidence one (expected the same constant). Both controls already passed; they did not expose the unequal-weight clipping defect.

### Correction

The risk implementation now:

- scales weights and losses by powers of two within its supported numerical range;
- accumulates products with multiplication residuals and compensated sums;
- computes confidence-zero CVaR through weighted-mean arithmetic, not a clipped tail;
- retains a compensated tail-mass remainder and the residual of 1-alpha;
- preserves low mass components at a fractional boundary;
- rejects unsupported dynamic range or severe numerical conditioning rather than returning an unqualified number;
- checks the transformed optimizer objective against the independently recomputed original scenario risk measure.

The conditioning check is an engineering guard, not a rigorous interval/error certificate. No exact-arithmetic or all-finite-input guarantee is claimed.

### Executed independent checks

| Case | Independent expected result | Current result |
| --- | --- | --- |
| Unequal raw weights [3,2,5], losses [10^17,-1,-6*10^16], confidence 0 | -0.2 | -0.2 |
| Losses [3*10^16,-1,-2*10^16], same weights, actual binary f64 confidence 0.05 | -0.15209352501972861 | Matches; all six input permutations agree |
| 128 small integer-weight tail cases | Exact integer mass accounting for rational confidence levels | Within predeclared tolerance |
| Equal weights at 1, f64::MAX and minimum subnormal | Mean of [2,4] is 3; upper half-tail is 4 | Matches |
| Zero-weight maximum-f64 outlier with tiny active constant | Active constant is unchanged | Matches |
| Expected-loss optimization with two maximum-f64 weights | Same normalized decision as equal weights; x=2, expected loss 0 | Matches |
| Extreme signed losses whose conditioning exceeds the supported range | Explicit numerical failure | No fabricated successful risk value |

The fractional-confidence oracle used Python Decimal at precision 100 with Decimal.from_float for the confidence. Using decimal 0.05 as though it were exactly the binary input would create a misleading reference on such highly conditioned data.

## Confirmed defect 2: strict unit-variant decoding

Serde's internally tagged unit expected_loss variant accepted a confidence field even when the enum had deny_unknown_fields. A caller could therefore provide a configuration intended for a tail-risk model and receive expected-loss behavior.

The public API/serialization remains compatible, but deserialization now uses a strict helper with an empty struct variant. Extra fields on expected_loss, conditional_value_at_risk, Scenario, ScenarioProblem and BoxUncertainty are rejected. The review test first reproduced the ignored unit-variant field, then passed after the correction.

## Mathematical assumptions checked

### Finite-scenario optimization

The implemented loss in each scenario is affine in the decision values, and the supplied base constraints are deterministic linear constraints. Weights are finite and nonnegative with positive mass. CVaR uses the upper tail of losses and a confidence in [0,1). The formulation introduces a free threshold and nonnegative excess variables, minimizing threshold plus weighted excess divided by tail probability.

This is a finite supplied-scenario formulation. It does not supply sample generation, independent out-of-sample testing, a confidence guarantee, stage/recourse semantics or chance constraints. Equal or unequal scenario weights are not evidence that a user's scenarios represent a population distribution.

Returned decision_values excludes the formulation's threshold/excess auxiliaries; solve.values describes the transformed optimization model. Callers must not treat that longer transformed vector as an original workbook decision range.

### Independent coefficient boxes

For a selected <= row, the implemented counterpart is:

    nominal_coefficients * x + sum(radius_i * abs(x_i)) <= rhs

It is valid for independent symmetric coefficient intervals and either sign of x. It does not model an uncertain right-hand side, correlated/ellipsoidal/budgeted sets, or arbitrary robust nonlinear models. Nonnegative continuous absolute-value auxiliaries enforce both sign inequalities.

The negative-coordinate control minimizes x under x+2*abs(x)<=3 and [-10,10], yielding x=-3 and absolute auxiliary 3. A second independent pre-existing control robustifies -x<=4 on [-10,0] and yields x=-2. Inputs are not modified.

## Resource, cancellation and validity review

| Area | Checked behavior | Remaining boundary |
| --- | --- | --- |
| Scenario compilation | Checks future variables/rows/terms, base indices/names/coefficients, confidence and memory estimate before copying/expansion | Estimates are not measured process-RSS limits; very large permitted input processing remains work |
| Robust counterpart | Counts +k variables, +2k rows and +5k terms for k positive radii; validates selected rows, dimensions and final limits before clone | Only <= coefficient boxes supported |
| Weighted tail helper | Fixed one-million-observation cap, bounded reserves and in-place unstable ordering | No caller-specific memory option for this standalone helper |
| Original risk measure | Recomputed losses/mean/CVaR are finite and checked against the optimizer objective | Error/range checks are floating point, not an exact rational certificate |
| Manifest | JSON streams into a byte-limited diagnostic hash writer | Byte bound is not a total application-memory quota |
| Native jobs | Four active jobs, structured failure/cancellation, panic containment and slot recovery | Arbitrary user evaluator work/allocations are the evaluator's responsibility |
| Completed results | Shared Arc result objects; repeated snapshots share the same allocation | Callers retaining handles/results retain their memory |
| Seeds | Maximum u64 and 2^53+1 serialize as decimal strings and round-trip exactly | Integer JSON input is accepted for compatibility; upstream client rounding cannot be repaired |

Pre-existing fixes were rechecked: escaped 4096-byte model names can expand beyond the manifest budget without creating a complete oversized JSON buffer; oversized CVaR/robust transformations fail their preflight; invalid base coefficients do not reach model copying; tiny constant tails and original opposing-coefficient cancellation controls remain valid.

## Native job lifecycle evidence

The added test starts four actual simulation workers and uses channels to hold all four inside their first evaluator call. It confirms that a fifth job is refused while those workers are active, rather than relying on a race with a very short solve.

After release, one evaluator deliberately panics, one job is cancelled, and two complete. The test checks:

- the panic becomes job_panic/Failed;
- cancelled work returns a cancelled result rather than fabricated completion;
- completed snapshots share their Arc allocation;
- a new real solve can start after slot guards release.

The SDK can catch unwinding evaluator panics on the tested native build. It cannot promise to recover from process abort, allocation abort, a panic-abort build or a user evaluator that never returns. Cancellation is cooperative at implemented polling points; hard real-time interruption of arbitrary evaluator code is not provided.

OS thread-creation failure, ID exhaustion and poisoned-lock branches were source-inspected but not forced in runtime tests. Actual WASM job behavior was not run; the native thread API explicitly rejects that host instead of claiming an asynchronous browser implementation.

## Reproducibility manifest assessment

The manifest stores model fingerprint, schema/package version, options/seed and platform. The FNV-1a-64 fingerprint is explicitly diagnostic. It is not authentication, a collision-resistant authorization token, a workbook revision guard or an algebraically canonical model identity. Reordering equivalent terms can legitimately change it.

The manifest does not yet provide a complete reproducibility bundle: compiler/build identity, host workbook revision, run identity and a corresponding simulation manifest remain separate work. Those omissions are classified as partial coverage, not silently satisfied by a hash.

## Executed review commands

Using CARGO_TARGET_DIR=$BUILD_OUTPUT\reference-review:

    cargo test --manifest-path solvercraft/Cargo.toml -p solvercraft --test runtime_review --locked --offline
    cargo test --manifest-path solvercraft/Cargo.toml -p solvercraft --locked --offline
    cargo clippy --manifest-path solvercraft/Cargo.toml -p solvercraft --all-targets --locked --offline -- -D warnings

The final additive runtime_review suite passed **10 tests**, zero failures/ignored. The full facade run immediately before its last additive control passed 80 tests (30 unit, 39 independent, 2 separate runtime lifecycle, 9 review). The consolidated final 265-test workspace run includes the final 10-test review suite. The nine-test intermediate run had 128 exact rational subcases inside one test function; those subcases are not added to Cargo's test count.

Formatting was applied to the two owned Rust files. Clippy passed. Incremental-cache hard-link warnings used the toolchain's copying fallback; they were environment warnings, not test or code-lint failures. No full Gridcraft/UI/WASM result is inferred from these commands.

## Source identities

The hashes below identify the locally reviewed source state rather than inventing a Git HEAD for a directory without Git metadata.

| File | SHA-256 |
| --- | --- |
| crates/solvercraft/src/risk.rs | dbb77ccce215af177bba14a37ccf4cdfe092bb67856757d5950d755c9d2f592f |
| crates/solvercraft/src/jobs.rs | 063dbf65c6f7e32fd4f83124ab4bc02e1da9cc7b2b36281adb84b42e57bda6e3 |
| crates/solvercraft/src/provenance.rs | 226760ca0908e7c09b85422990b2bc0517914655bf40854973eb7a9e9e0faff3 |
| crates/types/src/lib.rs | 133512f6ea8fd4f9e3740242c572eeac73153972a4d58ed4dbe186df687ae6a7 |
| crates/solvercraft/tests/runtime_review.rs | b8baada142d7979889fd5fb2983fa1a37406a4de1fc36c1aafb845498da46314 |

No external issue, pull-request or other publication was performed by this QA worker.

