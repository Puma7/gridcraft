# 12-week production planning

Open [the workbook](production-12-weeks.xlsx) in Gridcraft. Start on **Entscheidung** (Decision), then open **Data → Solvercraft → Refresh models**. Calculate and apply models 1 and 2 in turn. Run model 3 to compare all three plans under the same sampled conditions. **Create result sheet** records a snapshot with its source revision, solve status or trial count, and proposed cell values or simulation statistics. Create the report after applying a solution if both actions are needed.

The workbook starts with the solved plans and the delivered 2,048-trial reference run. Columns E–H on **Entscheidung** describe this fixed reference run. New simulations produce new result sheets. For the experiment, change **Annahmen!G16** from 26 to 20 and repeat the three models. The model treats the setup calendar and product-to-line assignment as inputs. Simulation applies stochastic yield losses to the fixed plan. It does not replan production or carry late orders as backlog.

| Plan | Nominal cost (EUR) | Mean on-time lot fraction | Probability of falling below 95% |
| --- | ---: | ---: | ---: |
| Minimum cost | 100,498 | 93.26% | 66.85% |
| Minimum cost with reserve | 103,238 | 94.52% | 51.71% |
| Current | 116,226 | 97.64% | 11.33% |

The probability is estimated from the simulated indicator, with a 95% Wilson interval in the workbook. Solvercraft checks 108 integer variables per model. An independently assembled SciPy/HiGHS MILP confirmed both optima at the original capacity and after the change (100,828 and 103,568 EUR). An independent stock-flow simulation recomputed all 2,048 shared trial paths for each of the three plans at both capacity levels. With demand volatility and both outage probabilities set to zero, all three plans deliver 100% on time in the simulation. The production test uses the headless Gridcraft MCP command transport, the same workbook command layer as the pane.


## Reproduce the checks

From the repository root, build `gridcraft-cli` and run the following with Python, NumPy, and SciPy installed:

```sh
python solvercraft/tests/e2e/verify_production_workbook.py target/debug/gridcraft-cli
```

The check also exercises workbook edits, stale apply protection, undo, report creation, save/reopen, empirical quantiles, and two additional seeds at 1,024 and 4,096 trials. Use the executable path ending in `.exe` on Windows.
