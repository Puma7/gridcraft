# Johnson population moment inversion

`solvercraft_math::johnson_moments` fits normal, shifted/reflected lognormal,
Johnson SU and Johnson SB distributions from mean, positive standard deviation,
skewness and **Pearson kurtosis**. This is genuine population moment inversion;
it is separate from Metalog quantile OLS/SPT fitting and does not substitute
random sample moments for population moments.

Implementation: `crates/math/src/johnson_moments.rs`. Public APIs are
`fit(mean, std_dev, skewness, kurtosis)`, `fit_with_options(..., options, control)`,
and `population_moments(distribution)`. A successful `JohnsonMomentFit` includes
the actual returned distribution, family, achieved population moments, maximum
relative residual, iteration/evaluation counts and method description.

## Family choice and bounded solver

Continuous nondegenerate moments require kurtosis > 1 + skewness². Equality and
violations return `infeasible_moments`, rather than a fabricated continuous fit.
For absolute skewness g, let v = 2 sinh(asinh(g/2)/3), q = v². The lognormal
boundary is 3 + q[16 + q(15 + q(6 + q))]. The zero-skew normal boundary has
kurtosis 3; moments above the lognormal boundary select SU and moments below it
select SB. Near-boundary fits still must pass the final requested-moment check.

SU uses closed Gaussian/sinh population moment relations in
w = exp(1/delta²) and r = tanh(-gamma/delta). Nested bounded bisections solve
asymmetry at fixed shape and then kurtosis at fixed skewness. SB uses
Y = logistic(m+sZ), with m = -gamma/delta and s = 1/delta, and deterministic
adaptive Simpson integration of four centered/scaled moments against the normal
density. Initial interval subdivisions include the logistic transition; Gaussian
Mills-tail estimates bound the omitted tails. Reflection avoids cancellation
near the upper endpoint. Compensated integration and stable logistic differences
reduce cancellation. Adaptive error estimates and independent tighter final
integration provide numerical checks, not an interval-arithmetic proof of the
entire quadrature error.

No successful return relies only on a shape-search residual. The rounded
distribution is validated and its actual moments are recomputed; mean, standard
deviation, skewness and kurtosis must meet the requested tolerance. This catches
affine scale/location overflow and loss of representable requested moments.
Negative-skew SB may return an explicitly reflected `ShiftScale` composition.

## Explicit numerical scope

Inputs require finite mean/std/skew/kurtosis, std > 0, abs(skew) <= 10,000 and
kurtosis <= 1e10. SU requires 1 < w <= 1e6 and abs(r) < 1. SB supports
abs(m) <= 40, 1e-5 <= s <= 128 and a Gaussian tail interval <= 40. Numerically
unresolvable variance, unbracketed shapes or convergence failures return explicit
errors. These are bounded numerical domains; mathematical feasibility alone
does not guarantee a successful numerical fit over every Johnson parameter.

`JohnsonOptions` bounds bisection iterations, shape evaluations and quadrature
nodes. Default relative tolerance is 1e-6; supported tolerance is [1e-9,1e-3].
Quadrature nodes are capped at 500,000 per shape evaluation and bounded depth 24;
the final postcheck uses the configured node cap too. Cancellation is checked
between shape evaluations, integration segments and within adaptive refinement.
No infinite retry or fallback to an empirical distribution is used.

## Evidence and compatibility limits

`crates/math/tests/johnson_moments.rs` has 5 executed Rust 1.95 tests. Nine
independent SU/SB parameter fixtures use SciPy 1.18.1 closed SU moments and
separate Gaussian integration (`scipy.integrate.quad`) for SB. Tests compare
population moments, recovered 5%/50%/95% quantiles, normal/lognormal boundaries,
Pearson feasibility, extreme numeric failure, evaluation limits and cancellation.
The independent SB oracle uses direct quadrature because SciPy's generic SB
moment integration was less accurate for one fixture.

The frozen PSI reference's `PsiMomentFit(mean,stdev,skew,kurtosis,...)` example
(10,20,4,41) identifies raw/Pearson kurtosis. This module preserves that convention.
Other frozen source tables call normal kurtosis 0 (excess); no implicit change
to sample/theoretical `PsiKurtosis` conventions is made. PSI properties,
outside-simulation defaults, optional arguments and complete worksheet import
remain separate open acceptance requirements. A full R046/PSI parity claim is
not established by this module.

Primary method references: Johnson (1949),
https://doi.org/10.1093/biomet/36.1-2.149; Hill, Hill and Holder (1976),
https://doi.org/10.2307/2346692; Tuenter (2001),
https://doi.org/10.1080/00949650108812126. The equations and bounded inversion
were implemented independently. No AS99 source, vendor code, licensed tables or
reference-manual prose is copied.
