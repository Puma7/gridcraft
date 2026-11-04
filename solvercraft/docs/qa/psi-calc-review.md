# R081: tested Calc policy for the nineteen imported PSI cores

Checkpoint: 2026-11-01, active local source after the separately frozen final5
checkpoint. Scope is the actual `gridcraft-calc` decoder/evaluator and its own
ordinary-cell contract. This is not a claim of vendor display-state emulation,
the entire PSI catalog, or completed host/browser release verification.

## Executed checks

`cargo test -p gridcraft-calc eval::solvercraft::tests --lib --offline --locked`
passed **21/21** selected tests on Windows/MSVC Rust1.95. The same package's
all-target Clippy with `-D warnings` passed. Targeted rustfmt covered only the
owned source/test files. No Cargo manifests/locks, reference JSON, global format
changes, or display-state architecture were introduced.

Source: `crates/calc/src/eval/solvercraft.rs` and its new
`crates/calc/src/eval/solvercraft/tests.rs`. The public decoder remains
`worksheet_distribution(name, args) -> Result<Option<WorksheetDistribution>, CellError>`,
returning the canonical distribution and its ordinary-cell nominal value.
No parameter-decoding defect was reproduced in the nineteen tested cores.

Each uniquely named `psi_*_default_override_domain_and_independent_parameters`
test performs all of the following for its one form:

- Checks its analytic ordinary-cell nominal and a separate independent CDF
  anchor that distinguishes parameter order/convention, even when means agree.
- Evaluates the actual formula at two captured times; the result follows the
  deterministic SC contract.
- Supplies an explicit trailing SC nominal, verifies the resulting cell value,
  and verifies the CDF and four addressed trial samples are unchanged.
- Checks an individual invalid required-parameter case, the same invalid case
  with a valid nominal override, missing/extra arguments, nonfinite nominal and
  propagation of an existing reference error.

| SC.PSI suffix and required arguments | Ordinary nominal | Independent CDF anchor |
|---|---|---|
| UNIFORM(-3,9) | 3 | F(0)=1/4 |
| INTUNIFORM(2,8) | 5 | F(4)=3/7 |
| NORMAL(7,2) | 7 | F(9)=Phi(1) |
| LOGNORMAL(10,4) | 10 | F(10/sqrt(1.16)*exp(sqrt(log(1.16))))=Phi(1) |
| LOGNORM2(0.7,0.4) | exp(0.78) | F(exp(1.1))=Phi(1) |
| TRIANGULAR(0,2,9) | 11/3 | F(1)=1/18 |
| PERT(0,1.5,6) | 2 | F(3)=13/16, independently Beta(2,4) |
| BETA(2,5) | 2/7 | F(1/2)=0.890625 |
| GAMMA(2,3) | 6 | F(3)=1-2/e |
| EXPONENTIAL(4) | 4 | F(4)=1-1/e, PSI argument is mean |
| WEIBULL(2,3) | 3*sqrt(pi)/2 | F(3)=1-1/e |
| BERNOULLI(0.3) | 0.3 | F(0)=0.7 |
| BINOMIAL(5,0.2) | 1 | F(0)=0.8^5 |
| POISSON(2.3) | 2.3 | F(0)=exp(-2.3) |
| NEGBINOMIAL(3,0.25) | 9 failures | F(0)=0.25^3 |
| GEOMETRIC(0.25) | 3 failures | F(0)=0.25 |
| DISUNIFORM({1,4,9}) | 14/3 | F(4)=2/3 |
| DISCRETE({1,4,9},{2,3,5}) | 5.9 | F(4)=1/2; weights are normalized |
| CUMUL(0,10,{2,6},{0.2,0.7}) | 4.6 | F(4)=0.45 |

The other two tests count Calc RNG calls (zero for these deterministic nominal
formulas) and check existing scalar coercion/override rules. An entirely absent
nominal uses the analytic policy. An explicitly present empty nominal is zero
under Calc numeric coercion; a numeric string or boolean follows the same
ordinary coercion. Multi-value scalar arrays are rejected, single-cell arrays
accepted, and the existing Uniform/Triangular/Pert nominal bound checks remain.
An override does not bypass invalid distribution parameters.

## Source comparison and genuine unknown contracts

Pinned public source: 2025Q1 reference,
SHA256 `2a166ac37367f2db870808d590c41ee8310c1676aa3a73164193e999d3d333c6`.
Page130 describes a random pre-run value, and p319 names several such families.
Page198 distinguishes a selected trial, the mean of normal completed trials,
and Base Case; absent Base Case falls back to the sample mean. It does not
establish one universal display-mode default. Page357's Bernoulli pre-run0.7
example conflicts with the general random statement; the [official PsiStatic
help](private compatibility reference)
also retains that example. The exact universal vendor pre-run policy remains
unestablished.

SC intentionally uses its deterministic ordinary value. Its analytic mean is
not a completed simulation's sample mean. The trailing SC numeric override is
neither PsiStatic nor PsiBaseCase, and does not implement ValueToDisplay. The
importer should accept only its advertised exact vendor core arities and expose
the lossy SC nominal policy/warnings separately. It must not reinterpret a
vendor property or extra positional value as that SC extension.

The confirmed host issues—SC.PSI recognition in the simulation IR and nested
stochastic imports that discovery cannot bind—are handled by the host owner
with actual import/discovery/simulation tests. This Calc checkpoint alone does
not prove that chain or a browser run. Nothing here adds undocumented vendor
defaults or changes the frozen reference scope.
