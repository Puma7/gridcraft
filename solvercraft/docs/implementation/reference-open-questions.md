# Source-backed compatibility questions

All page references below identify the frozen public **2025 Q1** guide,
SHA-256 `2a166ac37367f2db870808d590c41ee8310c1676aa3a73164193e999d3d333c6`.
These questions concern exact vendor compatibility, not a reason to omit
mathematically defined SDK capabilities. They have not been published externally.

| Function | Source and concrete discrepancy | Exact question | Implemented SDK policy |
|---|---|---|---|
| PsiPearson6Alt | PDF p310; current official help still describes two alternative claims, although alpha1, alpha2 and scale are three independent native parameters. The single `shape` label does not specify which shape. | Does the Alt call require three claims? If only two are supplied, which native parameter is fixed, at what default, and how are shape1/shape2 distinguished? Please supply a complete legal selector/value example for each supported combination. | `bounded(BetaPrime,...)` solves three explicit mathematical claims and postchecks extra independent CDF anchors. A two-claim vendor default is not invented. |
| PsiWeibullAlt | PDF p320; current official help repeats the BetaGen name and a one-percentile arity, despite Weibull's independent shape and scale. | What is the actual required claim count and selector vocabulary? Is there an implicit fixed shape/scale in a one-percentile call? Which two-percentile/native combinations are supported? | `weibull` solves two explicit, mathematically identifying claims, with original CDF checks; no literal one-claim PSI alias. |
| PsiNormalSkew | PDF pp305-306 (visually verified) uses width `(b-1)` in the median expression, whereas `a,b` are lower/upper percentile values. This fails translation equivariance when a differs from 1. Official help presents the same parameter geometry in images. | Is the intended median `a + (c+1)*(b-a)/2`? Please confirm the width correction and exact omitted-c behavior; is the fixed tail probability the displayed decimal constant or an exact normal ±3 probability? | `normal_skew` uses translation-consistent `b-a`, explicit c and the displayed tail constant. Tests check both skew directions and symmetric ±3-score behavior. It is separate from Azzalini skew-normal. |
| PsiAPARCH11 | PDF p344 prints a logarithmic-variance left side with the power-scale right side of standard APARCH. | Is the intended recurrence `sigma_t^delta = omega + alpha*(abs(epsilon)-gamma*epsilon)^delta + beta*sigma_prev^delta`, or does the implementation actually exponentiate that right side? | Published APARCH power-scale recurrence; tests verify delta2/gamma0 equals GARCH and leverage changes conditional scale. |
| PsiAR2 | PDF p341 prints a variance expression omitting the autoregressive covariance contribution. | Is the reported theoretical variance intended to be the Yule-Walker expression? Does the actual generator follow the stated centered AR(2) recurrence? | Centered recurrence with explicit startup; independent 6,000-trial test uses Yule-Walker variance. |
| PsiMVResample | PDF p337 chooses row/column resampling according to output orientation; the p338 example describes a column for both illustrated orientations. | For a row-shaped array result, is one intact input row sampled? What happens when output length does not equal that row/column length? | Explicit `VectorOrientation::Row/Column`, intact source vectors; no guessed oversized-output behavior. |
| PsiMomentFit / statistics kurtosis | PDF p301 example(10,20,4,41) identifies Pearson/raw kurtosis; the normal table p305 reports zero. | Please confirm each function's kurtosis convention, including sample/theoretical PsiKurtosis and dimensional forms, rather than one global convention. | Johnson moment inversion uses Pearson; statistical function conventions are separately documented and are not silently changed. |

Myerson's PDF pp302-303 swap b/c roles between prose and formulas. The current
official Myerson help
explicitly describes bottom/median/top positional order and total tail default
0.5. The SDK uses those named percentile meanings; the frozen formula-label
conflict remains provenance, rather than blocking its mathematical constructor.

Optional/property composition and host outside-value policies remain separate
open R081 questions even where the mathematical core is fully determined. None
of these source questions is counted as an implemented unsupported function.
