# Original workbook audit follow-up

This run revisited the 18 models left open by the [first compatibility audit](original-workbook-compatibility.md) and reran the 23 passing models. The original XLSX and XLSM files were read through the native Gridcraft MCP transport. Feasible candidates were applied in memory and undone. The source files were never saved and the workbook macros were never executed. HiGHS checked the translated linear or integer models independently.

## Changes and independent checks

- Fixed-input PRICE and DURATION now use the Gridcraft formula evaluator. The admitted scalar subset includes nested DATE and arithmetic over independent numeric inputs. A decision-dependent input or a cell containing an unproven formula cannot be frozen as a coefficient. A separate rational-arithmetic cash-flow calculation reproduced Bond2’s durations and enumerated the feasible two-asset vertices. Its optimum return was 0.09067473522085528. For Bond3, discounted cash flows supplied the prices and a separate integer program used the original coupon obligations. It found a cost of 280781.1564437513 with counts [0, 2, 165, 35, 0].
- When phase I cannot prove infeasibility using the original variable box, the certificate checker now tries bounds implied by original rows. It rounds the interval endpoints outward and verifies the signed separation using the original coefficients. These bounds are used only for certification. The phase-I residual alone still cannot establish infeasibility.
- Blend1 now completes at 9.5, agreeing with HiGHS. Alloc2 now finds a feasible candidate at 23311.5, agreeing with the HiGHS optimum of the translated model. Solvercraft reaches its iteration limit with a bound of 23280.833333333332. It reports a best-found candidate and does not claim optimality.

## Previously open models

| Workbook | Sheet | Outcome | Detail |
| --- | --- | --- | --- |
| Purchasing_Examples.xlsx | Invent1 | Feasible candidate | 3141.70777242, iteration_limit |
| Purchasing_Examples.xlsx | Invent2 | Refused | Nonlinear integer model outside the supported structural class |
| Purchasing_Examples.xlsx | Media | Refused | Unrepresentable coefficient division |
| Production-Examples.xlsx | Alloc2 | Feasible candidate | 23311.5, iteration_limit |
| Production-Examples.xlsx | Blend1 | Optimal and applied | 9.5, matched HiGHS |
| Production-Examples.xlsx | Blend2 | Refused | Final polynomial coefficient is not exactly representable |
| Production-Examples.xlsx | Process | Refused | Unrepresentable coefficient division |
| Investment-Examples.xlsm | Markowitz | Refused | Unsupported optimization function `_XLL.QUADPRODUCT` |
| Investment-Examples.xlsm | Full Markowitz | Refused | Unsupported optimization function `VARP` |
| Investment-Examples.xlsm | Efficient Frontier | Refused | Legacy simplex method is incompatible. Auto had separately failed to prove convexity. |
| Investment-Examples.xlsm | Sharpe | Refused | Unsupported optimization function `INDEX` |
| Investment-Examples.xlsm | Bond2 | Optimal and applied | 0.0906747352209, matched HiGHS |
| Investment-Examples.xlsm | Bond3 | Optimal and applied | 280781.156444, matched HiGHS |
| Investment-Examples.xlsm | Bond4 | Refused | Final polynomial coefficient is not exactly representable |
| Finance-Examples.xlsx | Finance | Refused | Final polynomial coefficient is not exactly representable |
| Finance-Examples.xlsx | Invent1 | Feasible candidate | 3141.69777552, iteration_limit |
| Finance-Examples.xlsx | Invent2 | Refused | Nonlinear integer model outside the supported structural class |
| Finance-Examples.xlsx | Lockbox | Refused | Final polynomial coefficient is not exactly representable |

The repeat sweep found 26 optimal results, 3 feasible candidates without a proof of optimality, and 12 explicit refusals.

## Additional inventory references

Both Invent1 sheets have the same continuous minimum of approximately 3141.69759146444. The objective is a sum of strictly convex positive-quantity terms. Solving the first-order conditions for the single storage multiplier gives the independent reference. Both Solvercraft candidates remain above it.

For both Invent2 sheets, an exact rational enumeration of the positive integer domains under the two resource limits gives quantities [23, 30, 29, 21] and cost 44867885/14007, about 3203.247304918969. An unconstrained suffix minimum was used only to prune candidates whose cost could not improve the incumbent. This is an independent reference for the refused nonlinear integer cases.
