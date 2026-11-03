# Qualification of the original workbook corpus

All 41 saved optimization models from six external workbooks now return a feasible optimal result. This is the same corpus as the [first compatibility audit](original-workbook-compatibility.md) and the [18-model follow-up](original-workbook-followup.md). This pass qualifies all 15 remaining models and repeats the 26 previously passing ones.

The native Gridcraft run opened each original XLSX or XLSM file, imported the saved Solver metadata, ran a Solvercraft job, applied the solution to the original worksheet formulas and undid the edit. The external files were never saved and macros were never run. HiGHS independently solved the translated matrix for each of the 33 LP or MILP models. All 15 new optima also have independently reconstructed references. The source worksheet evaluator checks the applied values.

## Newly qualified models

| Workbook | Sheet | Verified objective | Independent reference |
| --- | --- | ---: | --- |
| Purchasing_Examples.xlsx | Invent1 | 3141.697591465 | Strictly convex KKT calculation at 85-digit precision |
| Purchasing_Examples.xlsx | Invent2 | 3203.247304919 | Exact rational finite-domain search |
| Purchasing_Examples.xlsx | Media | 13875 | Exact spend-to-audience ratios |
| Production-Examples.xlsx | Alloc2 | 23311.5 | Exact integer-scaled dynamic program on the time-expanded network |
| Production-Examples.xlsx | Blend2 | 41.5 | Exact rational vertices for every feasible opening subset in each period |
| Production-Examples.xlsx | Process | 1020 | Exact lower bound from machine capacities and mandatory material assignment |
| Investment-Examples.xlsm | Markowitz | 0.003033052884615 | Exact rational face enumeration of the original quadratic |
| Investment-Examples.xlsm | Full Markowitz | 0.0002901403335238 | Independent population covariance and rational face enumeration |
| Investment-Examples.xlsm | Efficient Frontier | 0.01802551020408 | Exact rational face enumeration with the original return requirement |
| Investment-Examples.xlsm | Sharpe | 0.09128946627364 | Independent regression and risk-cap support optimization |
| Investment-Examples.xlsm | Bond4 | 263003.9670249 | Rational cash flows and independently formulated HiGHS integer program |
| Finance-Examples.xlsx | Finance | 16420 | Exact enumeration of 63,759 purchase triples with one-month reinvestment |
| Finance-Examples.xlsx | Invent1 | 3141.697591465 | Strictly convex KKT calculation at 85-digit precision |
| Finance-Examples.xlsx | Invent2 | 3203.247304919 | Exact rational finite-domain search |
| Finance-Examples.xlsx | Lockbox | 512000 | Exact enumeration of 63 nonempty opening subsets |

## Implementation scope

- Legacy imports use an exact extended affine formulation when the ordinary compiler refuses an unrepresentable coefficient. Auxiliary equalities preserve the original arithmetic and same-scale aliases carry exact product residuals. Direct requests retain the strict compiler by default and can opt in with `extended_affine: true`. Results project to original workbook cells. LP sensitivity remains limited to non-extended models.
- The simplex fallback detects repeated bases and checks an original-space dual bound. It repairs small negative basic residuals only within the existing feasibility tolerance and independently certifies a repaired optimum. Branch-and-bound retains the conservative bound where it differs from the candidate objective.
- The inventory route recognizes positive separable reciprocal costs under supported linear resource limits and obtains a certified bound. Bounded portfolio face searches cover small quadratic simplex models and supported positive-definite risk caps. The face search certifies the genuinely indefinite portfolio examples globally.
- Lockbox preserves the literal zero in the original `F39` cell. The cost 512000 and opening decision refer to that exact workbook. Replacing the zero with a formula changes the model.

## Result count by file

| Workbook | Optimal / tested |
| --- | ---: |
| Scheduling_Examples.xlsx | 7/7 |
| Purchasing_Examples.xlsx | 6/6 |
| Distribution-Examples.xlsx | 6/6 |
| Production-Examples.xlsx | 7/7 |
| Investment-Examples.xlsm | 8/8 |
| Finance-Examples.xlsx | 7/7 |
