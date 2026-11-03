# Reproducible conditional time-series simulation

The Rust SDK implements Gaussian-innovation AR(1), AR(2), MA(1), MA(2),
ARMA(1,1), ARCH(1), GARCH(1,1), EGARCH(1,1), APARCH(1,1), and deterministic
time-by-trial SIP matrix replay. This is executable simulation, with explicit
startup state and checked recurrences. It is not a fitted forecasting model,
a stationary random initialization, or a claim of complete PSI worksheet parity.

Implementation: `crates/math/src/time_series.rs`; executed acceptance tests:
`crates/math/tests/time_series.rs` (10 tests, Windows Rust 1.95).

## Public contract

`sample_path(spec, length, SeriesAddress { seed, binding, trial }, limits, control)`
returns values, the raw process conditional standard deviations, normal shocks,
step count, stream version and startup description. `value_at` replays the
immutable prefix to a zero-based time index. Use `sample_path` for bulk work.
`simulate_with_shocks` accepts an explicitly conditioned sequence of finite
standard-normal shocks; it does not certify that external numbers are random.
`replay_tssip(matrix, trial_index, limits, control)` returns one entire column,
with rows representing time and columns representing trials. It consumes no RNG.

Every random shock is independently addressed by seed, binding, trial and time.
The version is `solvercraft-time-series-normal-v1/addressed-mix64-v2`.
Extending a path preserves its existing prefix. Separate trial/binding addresses
do not share mutable generator state. Full 64-bit addresses serialize as decimal
strings. The stream and startup conventions are Solvercraft contracts, not
claimed reconstructions of legacy PSI's generator or seed mapping.

Output index 0 is the first generated value after the supplied time-0 state.
For AR(2), `initial_latest` is Y0 and `initial_previous` is Y-1. MA startup errors
are innovations, not observations. Conditional-volatility initial observations
are converted to centered initial errors; GARCH/EGARCH/APARCH additionally need
a positive initial conditional standard deviation. There is no hidden warmup.

## Equations and parameter conventions

Write x = Y - mean and epsilon_t = sigma_t Z_t, with independent normal Z.
AR/MA coefficients multiply centered observations and innovations respectively;
the ARMA equation combines these terms. Innovation scales are nonnegative, so
zero is an explicit deterministic SDK case.

| Process | Conditional recurrence |
|---|---|
| ARCH | sigma_t² = omega + alpha epsilon_(t-1)² |
| GARCH | sigma_t² = omega + alpha epsilon_(t-1)² + beta sigma_(t-1)² |
| EGARCH | log sigma_t² = omega + alpha [theta Z_(t-1) + gamma (abs(Z_(t-1)) - sqrt(2/pi))] + beta log sigma_(t-1)² |
| APARCH | sigma_t^delta = omega + alpha (abs(epsilon_(t-1)) - gamma epsilon_(t-1))^delta + beta sigma_(t-1)^delta |

ARCH/GARCH/APARCH intercepts are positive variance/power intercepts. EGARCH's
log-variance intercept can be negative. GARCH scale calculation uses hypot;
APARCH uses logarithmic power sums. Unrepresentable scales/values and underflow
of required positive EGARCH/APARCH scales return errors.

With `require_stationary=true` (default), base AR coefficients must satisfy
the AR(1)/AR(2) stationarity triangle; ARCH requires alpha < 1, GARCH requires
alpha+beta < 1, and EGARCH requires abs(beta) < 1. MA stationarity does not require
invertibility. APARCH requires alpha E[(abs(Z)-gamma Z)^delta] + beta < 1, using
the analytic normal absolute moment. This APARCH check is a sufficient finite
delta-moment contraction condition; it is not labeled a complete classification
of strict stationarity. A stationary base recurrence does not make an integrated
postprocessed series stationary, and fixed startup states are conditional states.

## Postprocessing and source discrepancies

The SDK applies integration, then seasonality, then transformation in that fixed
order. Integration order 1/2 carries explicit initial level/difference. Seasonal
integration order 1/2 uses chronological previous-season arrays. Additive
seasonality uses an explicit zero-based phase. Exponential, square and identity
transformations add their shift after the transformation.

The frozen 2025 Q1 reference pages 340-344 document the PSI generator signatures;
pages 361-362 describe time-series properties. Their complete composition order,
omission rules and indexing/startup behavior are not established by this SDK.
No PSI time-series worksheet importer is claimed here.

Two mathematical discrepancies remain recorded rather than silently treated as
parity: the AR(2) printed variance formula omits covariance terms; the SDK's
population test uses the Yule-Walker variance
sigma²(1-phi2)/[(1+phi2)((1-phi2)²-phi1²)]. The APARCH reference prints a
log-variance left side with a power-scale right side; the SDK uses the published
APARCH power-scale equation above. Exact vendor behavior for these discrepancies
remains unverified.

## Resource and evidence scope

Lengths are capped at one million steps, seasonal periods at 100,000, APARCH
delta at [0.1,8], and asymmetry strictly inside (-1,1). Memory is preflighted for
three output arrays plus seasonal state. Step, memory and cancellation limits
are checked before allocation and during generation/replay. SIP input is capped
at one million cells, must be finite and rectangular. Serde configurations use
strict unknown-field rejection, including empty struct variants for no-op cases.

The 10 tests check independent two-step ARMA algebra; ARCH/GARCH/EGARCH variances;
the APARCH delta=2, gamma=0 identity and leverage; initial states and all
postprocessing families; prefix/index/trial/binding behavior; invalid parameters,
numeric overflow, budgets and cancellation; SIP column replay; strict JSON round trips and unknown-field rejection across all recurrence/postprocessing variants, full-width decimal addresses; and a deterministic
6,000-trial AR(2) population-variance comparison. The population comparison is
statistical evidence, not a proof of the RNG's distribution or vendor parity.

Primary algorithm references: Bollerslev (1986),
https://doi.org/10.1016/0304-4076(86)90063-1; Nelson (1991),
https://doi.org/10.2307/2938260; Ding, Granger and Engle (1993),
https://doi.org/10.1016/0927-5398(93)90006-D. Equations are implemented independently
in Rust. No vendor implementation, reference PDF, manual prose, or licensed code
is included in the repository.
