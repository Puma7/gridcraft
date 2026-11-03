# Original workbook compatibility audit

The five supplied workbooks have 34 saved models. Finance-Examples contributes seven follow-up models. The audit opened the original XLSX and XLSM files through the native Gridcraft MCP workbook interface. It imported each saved model with a unique name, validated it, solved with a 12-second job limit, applied every feasible result in memory, and undid that edit. Source files were opened without saving, and workbook macros were never run. The source files and vendor assets are external inputs.

The original import returned seven optimal results, one best-found candidate, one inconclusive run and 32 errors. With the compatibility changes it returned 23 optimal results, two best-found candidates, two inconclusive runs and 14 errors. HiGHS independently optimized the translated matrix for all 23 optimal results and agreed with each objective. Gridcraft also checked the objective and constraints through its original formula evaluator before applying. The independent matrix solve confirms the numerical solution to the translated model. It is separate from checking the import semantics.

Budget2 had a second independent check using exact rational arithmetic over the original cash flows and every one of the 64 binary portfolios. Fourteen portfolios were feasible. Investments 1, 5 and 6 were uniquely best, with a net present value of approximately 19.629046 million dollars. The independently calculated annual surpluses were 2, 15, 0, 8, 55 and 56 million dollars.

## Reproduce

Open each workbook using the Gridcraft CLI MCP `open_workbook` tool. Activate the listed sheet and run `solvercraft.model.importLegacy` with a distinct name for each sheet. Pass the returned request to `solvercraft.validate` and the returned model ID to `solvercraft.job.start`. Read the completed job with `solvercraft.job.result`. For a feasible result, `solvercraft.job.apply` recomputes the workbook using the original formula engine and `edit.undo` reverses the edit. Use the validated model’s linear coefficients, constraints, bounds and integer domains for a separate LP or MILP solver. Use HiGHS with a zero relative MIP gap for the comparisons below.

## Model results

| Workbook | Sheet | Result | Objective / reason |
| --- | --- | --- | --- |
| Scheduling_Examples.xlsx | Crew | Optimal, applied and matched HiGHS | 8775 |
| Scheduling_Examples.xlsx | Offices | Optimal, applied and matched HiGHS | 23 |
| Scheduling_Examples.xlsx | Sched1 | Optimal, applied and matched HiGHS | 5000 |
| Scheduling_Examples.xlsx | Sched2 | Optimal, applied and matched HiGHS | 158 |
| Scheduling_Examples.xlsx | Sched3 | Optimal, applied and matched HiGHS | 610 |
| Scheduling_Examples.xlsx | Hirefire | Optimal, applied and matched HiGHS | 1050000 |
| Scheduling_Examples.xlsx | Troops | Optimal, applied and matched HiGHS | 32950 |
| Purchasing_Examples.xlsx | Award1 | Optimal, applied and matched HiGHS | 3670 |
| Purchasing_Examples.xlsx | Award2 | Optimal, applied and matched HiGHS | 3670 |
| Purchasing_Examples.xlsx | Invent1 | Feasible incumbent, applied | 3141.707772, iteration limit |
| Purchasing_Examples.xlsx | Invent2 | Refused | Nonlinear integer objective outside the supported structural class. |
| Purchasing_Examples.xlsx | Media | Refused | Coefficient division cannot be represented exactly. |
| Purchasing_Examples.xlsx | Purchase | Optimal, applied and matched HiGHS | 74690 |
| Distribution-Examples.xlsx | Transport1 | Optimal, applied and matched HiGHS | 192750 |
| Distribution-Examples.xlsx | Transport2 | Optimal, applied and matched HiGHS | 145500 |
| Distribution-Examples.xlsx | Transport3 | Optimal, applied and matched HiGHS | 519700 |
| Distribution-Examples.xlsx | Knapsack | Optimal, applied and matched HiGHS | 100 |
| Distribution-Examples.xlsx | Facility | Optimal, applied and matched HiGHS | 210500 |
| Distribution-Examples.xlsx | Prodtran | Optimal, applied and matched HiGHS | 1566450 |
| Production-Examples.xlsx | ProductMix | Optimal, applied and matched HiGHS | 25000 |
| Production-Examples.xlsx | Alloc1 | Optimal, applied and matched HiGHS | 4665 |
| Production-Examples.xlsx | Alloc2 | Numerically inconclusive | HiGHS reference 23311.5, no Solvercraft certificate |
| Production-Examples.xlsx | Blend1 | Numerically inconclusive | HiGHS reference 9.5, no Solvercraft certificate |
| Production-Examples.xlsx | Blend2 | Refused | Final polynomial coefficient cannot be represented exactly. |
| Production-Examples.xlsx | Process | Refused | Coefficient division cannot be represented exactly. |
| Production-Examples.xlsx | Cutstock | Optimal, applied and matched HiGHS | 350 |
| Investment-Examples.xlsm | Markowitz | Refused | Unsupported optimization function `_XLL.QUADPRODUCT`. |
| Investment-Examples.xlsm | Full Markowitz | Refused | Unsupported optimization function `VARP`. |
| Investment-Examples.xlsm | Efficient Frontier | Refused | Imported method requires simplex. Auto also refuses because convexity is unproven. |
| Investment-Examples.xlsm | Sharpe | Refused | Unsupported optimization function `INDEX`. |
| Investment-Examples.xlsm | Bond1 | Optimal, applied and matched HiGHS | 0.08333333333 |
| Investment-Examples.xlsm | Bond2 | Refused | Unsupported optimization function `DURATION`. |
| Investment-Examples.xlsm | Bond3 | Refused | Unsupported optimization function `PRICE`. |
| Investment-Examples.xlsm | Bond4 | Refused | Unsupported optimization function `PRICE`. |
| Finance-Examples.xlsx | Finance | Refused | Final polynomial coefficient cannot be represented exactly. |
| Finance-Examples.xlsx | Budget1 | Optimal, applied and matched HiGHS | 22.74495357 |
| Finance-Examples.xlsx | Budget2 | Optimal, applied and matched HiGHS | 19.62904607 |
| Finance-Examples.xlsx | Invent1 | Feasible incumbent, applied | 3141.697776, iteration limit |
| Finance-Examples.xlsx | Invent2 | Refused | Nonlinear integer objective outside the supported structural class. |
| Finance-Examples.xlsx | Lockbox | Refused | Final polynomial coefficient cannot be represented exactly. |
| Finance-Examples.xlsx | Open | Optimal, applied and matched HiGHS | 3140 |

## Scope of the changes

Legacy import preserves sheet qualification and resolves absolute named ranges with sheet scope taking precedence. It reads the declared active constraint count and refuses incomplete active rows. Formula-dependent or relative names still require explicit review. The NPV path delegates to the worksheet evaluator after verifying that all arguments are fixed numeric literals or independent stored numeric cells. Models in which NPV depends on a decision variable still require structural support.

The numerical certificate rules remain in effect. Alloc2 and Blend1 reached branch subproblems without a valid original-model infeasibility certificate. The solver returned unknown feasibility and no optimum. Independent feasible optima for the imported models identify these as solver coverage gaps. A separate run of Efficient Frontier with automatic engine selection reached the convexity guard.
