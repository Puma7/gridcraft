# Typed reference parameterizations and executable function mapping

`crates/math/src/reference_functions.rs` supplies typed inverse parameterizations
for the mathematical cores of the 28 frozen Alt families and explicit native
location/bounds/order conversions. `crates/math/tests/reference_functions.rs`
has **40 executed tests**, including independent CDF formulas and additional
percentiles not used during fitting. Windows Rust 1.95 tests and all-target
Clippy passed. This does not establish PSI worksheet property/default parity.

`reference-implementation.json` has one row per frozen function, with source
hash/pages, actual executable test selectors, native/browser status, guarantee
and remaining gap. Its R046 scope is **111** distribution/Alt/custom/special/
time-series entries. The other 161 entries remain in the full-reference/R091 inventory, not as
additional R046 distribution requirements or an implicit mandatory R081 import list.

The final6 [original-scope audit](../qa/original-scope-audit.md) maps all 111
mathematical capability names to positive cases in the actual SDK 630 run
(73 test bodies, 13 source files) and records the source hashes/log lines. R046
has bounded mathematical acceptance. Exact Pearson 6Alt/WeibullAlt vendor arity
remains open; that does not erase the executed mathematical constructors.
R081 separately accepts only its nineteen advertised top-level conversion
forms/policies, with native and real-browser evidence and disclosed outside
value loss. It does not promote these typed constructors to worksheet aliases.

## Public mathematical contracts

`Claim` is a strict tagged enum for percentile probability/value, mean, variance,
standard deviation, location, scale, shape(s), lower/upper bound and mode. Native
selector meanings are explicit: e.g. Inverse-Gaussian `Scale` means lambda;
lognormal mean/std claims are arithmetic moments. Unknown fields, duplicate
selectors, invalid probabilities and inconsistent claims return errors.

| API | Covered reference cores | Method |
|---|---|---|
| `location_scale` | Normal, Uniform, Cauchy, Logistic, Laplace, HypSecant, Levy, MaxExtreme, MinExtreme Alt | Affine 2 x2 parameter equations, original CDF check |
| `scale_only` | Erf, Exponential, Rayleigh Alt | Scale/moment identities, original CDF check |
| `shape_only` | ChiSquare, Student Alt | Bounded CDF root; ChiSquare variance identity |
| `shape_scale` | Gamma, Pearson 5, Pareto, Pareto 2 Alt | Native/moment identities or bounded one-dimensional shape root |
| `location_shape_scale` | FatigueLife, Frechet, LogLogistic Alt | Three percentiles or native/percentile mixtures; affine elimination plus bounded shape root |
| `inverse_gaussian` | InvNormal Alt | Mean/variance/lambda identities or bounded shape-ratio root |
| `lognormal` | LogNormal Alt | Log-space identities; arithmetic-mean quadratic or bounded std root |
| `bounded` | BetaGen, Pert, Triangular Alt mathematical claims | Damped Gauss-Newton/Levenberg-Marquardt parameter equations, original claims postchecked |
| `bounded(BetaPrime,...)` | Mathematically identified Pearson 6 alternative | Three explicit claims, rather than inventing the frozen two-claim default |
| `weibull` | Mathematically identified Weibull alternative | Two explicit claims, rather than inventing the frozen one-claim default |

`burr12`, `dagum`, `fatigue_life`, `frechet`, `johnson_sb`, `kumaraswamy`,
`log_logistic` and `pareto` preserve the reference's native location, min/max and
parameter order. `normal_skew` implements the Myerson percentile geometry with
the fixed total tail probability, separately from Azzalini's `SkewNormal` DTO.
Its width correction and unresolved vendor documentation are explicit below.

## Numerical domains, budgets and guarantees

Closed-form solutions must validate the returned distribution and original CDF
anchors. One-dimensional roots are bracketed on positive shape [1e-4, 1e4] in log
coordinates, with finite grids, capped bisection, evaluation limits and cancel
checks. The first bracketed valid parameterization is the documented numerical
selection policy; no global uniqueness guarantee is made. Different inverse
formulations can be non-identifiable. In particular, a lognormal percentile plus
arithmetic mean can have two positive log scales, and a triangular CDF's single
branch cannot identify both bounds and its mode. These are explicit errors.

The small nonlinear fit uses at most four parameters, normalized values,
positive log-shapes/scales and a logistic interior mode. It solves damped normal
equations with checked pivots and bounds each update. Five deterministic starts,
the configured residual-evaluation limit (maximum 100,000), and iteration limit
(maximum 256) bound work. Normalized location is restricted to [-100,100], width
log to [-20, 20], Beta shapes to [1e-4, 1e4] and interior-mode logit to [-18, 18].
These limits are explicit numerical scope, not the entire vendor domain.

Successful numerical fits must preserve all original percentile claims, pass a
local Jacobian rank check and independently verify native parameters/moments of
the actual rounded returned distribution. Variance is checked in relative std
units, so a large location cannot hide a wrong tiny positive variance. A
regression covers that case. CDF checks allow 3e-7 to accommodate the legacy
normal CDF approximation. No numerical rank check proves global uniqueness.
Convergence failures, incompatible claims, ambiguous parameters, cancellation
and budgets return structured errors; they are not relabeled as successful fits.

BetaGen four-percentile tests compare extra anchors against the independent
Beta(2, 3) CDF polynomial. Pert tests use Beta(2, 4)'s polynomial and canonical
weight 4. Triangular tests cover both branches and explicitly reject three
single-branch percentiles by the inverse-CDF identity. FatigueLife tests use
Gaussian score geometry; InvNormal two-quantile tests use independent SciPy
constants. Other cases use tangent, exponential, power and normal identities.

The nonlinear parameter algorithm is implemented independently from the primary
method of Marquardt (1963), https://doi.org/10.1137/0111030. No vendor code,
licensed implementation, reference PDF or copied manual prose is included.

## Remaining compatibility questions

The frozen source and current official help leave Pearson 6Alt/WeibullAlt arity
underdetermined, and print a non-translation-consistent NormalSkew width.
[reference-open-questions.md](reference-open-questions.md) records exact sources,
questions and the SDK's explicit choices. The mathematical constructors do not
invent an undocumented PSI default or register these ambiguous names as worksheet
aliases. Optional properties, omitted/empty arguments, outside-simulation values,
cell metadata, fitting defaults and host integration remain separate R081 work.
