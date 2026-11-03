# Frontline parity audit

Scope is the frozen Frontline 2025 Q1 function inventory. All 272 entries were reconciled by ID, name and category with the [per-function implementation matrix](reference-implementation.json). The matrix links requirements, evidence and the remaining gap per entry. The workbook and UI milestones below have separate acceptance criteria.

| Family | Catalog entries | Bounded import | Open import |
| --- | ---: | ---: | ---: |
| continuous distribution | 77 | 22 | 55 |
| custom distribution | 6 | 3 | 3 |
| custom modeling | 2 | 0 | 2 |
| decision table | 2 | 0 | 2 |
| dimensional modeling | 12 | 0 | 12 |
| discrete distribution | 8 | 6 | 2 |
| distribution property | 31 | 0 | 31 |
| optimization modeling | 22 | 0 | 22 |
| rason conversion | 2 | 0 | 2 |
| sample statistics | 47 | 1 | 46 |
| simulation control | 3 | 0 | 3 |
| simulation time series | 10 | 0 | 10 |
| six sigma | 21 | 0 | 21 |
| special distribution | 10 | 0 | 10 |
| theoretical statistics | 19 | 0 | 19 |
| Total | 272 | 32 | 240 |

The bounded imports comprise 31 numeric distribution signatures plus the bare
zero-argument output marker. The latter may stand alone or be added to a numeric
formula. Named declarations and optional arguments require separate contracts.
The independent SDK math has individual core evidence for 111 entries. A core
test does not qualify the original workbook formula, its options or its UI.

## Workbook qualification

| Corpus | Verified status | Evidence or next gate |
| --- | --- | --- |
| Six original Frontline optimization files | 41 of 41 saved optimization models previously qualified | [Original workbook qualification](../qa/original-workbook-qualification.md). The six supplied files have no PSI calls in worksheet formulas. |
| Frontline College Fund and other original simulation files | Original files unavailable in the local corpus | [College Fund source lead](https://www.solver.com/risk-solver-app-testing-notes) returned HTTP 429 on direct retrieval. [Frontline documents further examples](https://solver.com/download-analytic-solver-platform) in the desktop package. Record source and hash when originals become available. |
| Authored XLSX regression | Bounded import and calculation test | Reads an XLSX, previews a uniform input and output marker, refuses a stale preview, converts supported cells, checks shared draws against analytic mean and variance, then undoes the conversion. |
| Authored UI regression | Preview and undo through the actual task pane | Scans with no cell edits, converts after confirmation and opens a simulation draft. |

## Highest-priority gates

1. Qualify original Monte Carlo workbooks before expanding signature mappings.
   Record every encountered PSI call, ordinary formula, named range and external
   dependency. Use closed-form or independently implemented statistical checks.
2. Prioritize output and sample statistics, then modifiers and correlations.
   Keep results tied to the run and keep any named, instance-specific or vendor
   display behavior explicit.
3. Exercise two-stage and simulation optimization separately from plain
   simulation. Compare candidates on common random numbers and evaluate the
   selected decision against independent scenarios.
4. Reconcile RASON conversion, decision tables and dimensional modeling as
   separate capability areas, with platform and workbook evidence for each.
