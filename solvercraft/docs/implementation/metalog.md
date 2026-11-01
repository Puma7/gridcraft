# Metalog and verified PSI constructors

Updated 2026-10-31. These are workbook-independent numerical APIs. They do not imply full worksheet/default/property compatibility with every PSI function in the frozen reference catalog.

## Metalog public API

`math::metalog::Metalog` holds `coefficients`, `lower` and `upper`. `PreparedMetalog::new(&Metalog)` checks the model and returns a reusable prepared distribution with `quantile`, `cdf`, `log_density`, `density`, `model` and `certificate` methods. `fit_cdf(terms,x_values,y_values,lower,upper)` performs Householder-QR ordinary least squares on CDF point pairs. `fit_spt(lower,upper,[q1,q2,q3],p)` uses probabilities `[p,0.5,1-p]`. Both return `MetalogFit` containing the model, observation count, latent-space RMSE, explicit method name and monotonicity certificate.

The basis uses Keelin's original 2016 coefficient order. With `t=p-1/2` and `z=log(p/(1-p))`, its first terms are `1,z,t*z,t,t^2,t^2*z,t^3,t^3*z,...`. This is not silently replaced by a later reordered basis. The SDK caps coefficients at 30; the frozen PSI reference describes only 2-10. An eventual PSI import must apply the reference signature limit separately.

The latent quantile `M(p)` is used directly for an unbounded model, as `lower+exp(M(p))` for a lower-bounded model, and through a logistic map to `[lower,upper]` for a bounded model. Upper-only bounds are explicitly unsupported, matching the frozen PSI subset. Both finite bounds must have a representable positive width. Unbounded endpoint quantiles are errors; finite endpoints are returned exactly at p=0/1. An interior value that overflows or rounds to a support endpoint is a numerical error, rather than an invented atom.

CDF and density invert the monotone latent quantile with bounded bracketing and bisection in logit space. The location coefficient is removed before inverse comparisons, so a large offset does not erase a small change in the argument. Log-density uses the chain rule in log space to avoid gratuitous tail underflow. Like existing scalar log-density APIs, values outside support return negative infinity; malformed inputs and unrepresentable arithmetic are errors.

## Global monotonicity certificate

Validity is not inferred from a probability grid. The implemented subset has an analytic sufficient certificate on the derivative with respect to z over the entire real line. Let `v=p*(1-p)` and `t=p-1/2`. Then `|t|<=1/2`, `v<=1/4`, and `|z|*v<=1/4`. The last inequality follows from `v=1/(4*cosh(z/2)^2)` and `cosh(z/2)^2 >= 1+z^2/4 >= |z|`.

These give the following global absolute derivative bounds for each basis term:

| Basis term | Bound on derivative with respect to z |
|---|---|
| `z` | exactly 1 |
| `t*z` | 3/4 |
| `t` | 1/4 |
| `t^k` | `k*2^(1-k)/4` |
| `t^k*z` | `2^(-k)+k*2^(1-k)/4` |

The positive `a2` contribution must exceed the outward-rounded sum of adverse coefficient bounds. A positive coefficient of the `t` term contributes nonnegatively and need not be subtracted. A positive certified margin proves a strictly increasing latent quantile and therefore a proper continuous distribution under each supported increasing transform. The check is deliberately sufficient, not necessary: many mathematically valid Metalogs can return `metalog_uncertified`. No coefficient set that merely looks valid at sampled points is accepted. The certificate is a numerical implementation of these stated inequalities, not a complete symbolic feasibility decision procedure.

## Fitting and moment fitting

CDF fitting requires ordered finite x values, strictly increasing probabilities in (0,1), at least as many observations as terms, and at most one million design-matrix cells. Responses are centered before scaling so large offsets do not dominate scale estimation. Householder QR, finite arithmetic checks and a diagonal conditioning threshold avoid treating a rank-deficient design as a reliable fit. The fitted coefficient vector must pass the same global monotonicity check as a direct model. The reported RMSE is in the latent transformed coordinate used by OLS. Neither OLS nor SPT is labeled maximum likelihood.

The frozen `PsiMomentFit` function is a **Johnson-system moment fit**, not a Metalog fit. The separate `moment_fit_supported(mean,stddev,skewness,kurtosis)` API implements exact normal and shifted/reflected lognormal subfamilies only. It analytically checks the skewness/kurtosis relation, reconstructs the rounded parameters and verifies that the requested mean and standard deviation remain representable. The source example `(10,20,4,41)` belongs to a shifted lognormal subfamily and is covered. General Johnson SU/SB inverse moment fitting returns `unsupported_moment_family`; no OLS output is misrepresented as that feature.

## PSI parameter constructors

`math::psi_aliases` supplies checked constructors for arithmetic `PsiLogNormal`, log-parameter `PsiLogNorm2`, mean-parameter `PsiExponential`, generalized/subjective Beta, ERF, double triangular, minimum extreme value, cumulative/descending cumulative, general, histogram and weighted/equal-probability custom distributions.

The custom-CDF constructors interpolate explicitly supplied cumulative probabilities and use a mixture of uniform intervals. They support at most 32 nonzero-mass intervals; zero-width intervals with positive probability are rejected because their PSI endpoint-atom meaning is unresolved. Equal interior cumulative probabilities and duplicate x knots are rejected for `PsiCumul`. Positive probability underflow during normalization is explicit. Subjective Beta rejects a symmetric mean=mode case whose concentration is not uniquely identifiable. These restrictions are visible subsets, not hidden guesses about vendor defaults.

## Executed evidence and remaining gaps

The eleven public tests in `crates/math/tests/metalog.rs` cover independent logistic quantile/CDF/density identities, a bounded uniform identity, a lower-bounded log-logistic identity, an independently expanded ten-term expression, coefficient recovery from independently generated CDF data, large-offset inverse and OLS cases, SPT recovery under three bound modes, global-certificate rejection, numerical endpoint behavior, density integration and the separate moment-fit subfamilies. Seven tests in `psi_aliases.rs` cover parameter conversions and custom probability mass. The catalog integrity tests do not substitute for these numerical tests.

Remaining gaps include complete necessary-and-sufficient Metalog feasibility, all valid coefficient sets allowed by Frontline, full Johnson SU/SB moment inversion, all alternate parameter selectors, worksheet fitting/import/property/default semantics, and per-function browser execution. R046 remains partial until each frozen function/domain has appropriate tests and evidence.

Primary mathematical source: T. W. Keelin, [The Metalog Distributions](https://doi.org/10.1287/deca.2016.0338), Decision Analysis 13(4), 243-277, 2016; [author's equations and feasibility notes](https://www.metalogdistributions.com/equations.html). The API ordering and PSI scope are cross-checked against public 2025 Q1 reference pages 297-301. Code and the dominance certificate implementation are independently written; no vendor code or feasibility table is copied.
