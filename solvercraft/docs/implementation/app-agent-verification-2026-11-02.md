# Standalone app and on-demand agent integration verification

Executed: 2026-11-02 on Windows x86-64 with Rust/Cargo 1.95.0. This delivery adds
a native Solvercraft application, an independent stdio MCP entry point, shared
host integration documentation and a pinned ArtCraft Launcher submission.
It does not deploy a hosted service or start a network listener.

## Executed checks

| Check | Actual result |
|---|---|
| Complete SDK workspace tests | 645 passed, 0 failed, 0 ignored |
| SDK format and all-target Clippy, warnings denied | Passed |
| Complete Gridcraft CI | All 10 stages; 2,911 passed, 0 failed, 1 existing ignored timing test |
| Standalone MCP subset, included in SDK total | 15 passed: 7 real stdio-process cases and 8 protocol/resource cases |
| Native app subset, included in host total | 4 contracts: LP preview/apply/Undo, saved-model XLSX roundtrip and real seeded simulation, selected model, arguments |
| Shared Solvercraft pane scoped checks | 11 passed, including compiled-model equality after readable formatting and advanced-metadata retention |
| Gridcraft-update host-disable regression | Passed; default Gridcraft behavior retained |
| Native optimized app build | Passed; `Solvercraft 0.14.2` executed |
| Release CLI/MCP invocation | CLI capabilities and MCP version executed successfully |
| Actual offscreen app rendering | Generated and visually inspected |

The SDK total includes the original 630 checks and the 15 new MCP checks; the
subsets above are not added again to the full totals. Browser execution evidence
remains the separate, source-bound [final7 record](verification-final7.md). The
fresh host CI includes WASM compilation; this session does not claim a new actual
Chrome run or a native-window interaction test on every operating system.

The standalone app reuses the existing command-driven workbook interface.
Its production example solves to decision `[2,6]`, objective `36`. Result preview
leaves the live workbook unchanged; explicit apply is one Undo step. The margin
model performs actual seeded sampling and survives the saved-model roundtrip.
Simple model fields render as readable cell references and constraints. Advanced
bounds/kinds, metadata and ambiguous formulas retain their JSON representation.

## Agent contracts and corrected boundary cases

The standalone `solvercraft-mcp` process exposes capabilities, algebraic solve,
simulation, portable-model execution and get/cancel/forget lifecycle tools. It
has no Gridcraft, HTTP, account or network-listener requirement. Its client owns
the process and its ephemeral retained results; EOF cancels owned work and
triggers bounded shutdown.

Actual process tests exercise LP objective 36, a constant simulation output 21,
portable object/property bindings and lossless revisions/seeds, qualified local
nonlinear results, protocol errors, quotas, cancellation/deadline and EOF.
The numerical SDK engines themselves are unchanged by this integration.

Independent review reproduced three result-size admission failures in an early
MCP candidate: wide streaming summaries, large fixed-decision vectors and
portable execution could finish but exceed a small configured response quota.
The final adapter rejects these known oversized cases before admission and
reservation. Independent final-release tests verified all three rejection cases
and all three scalar retrieval controls under a 65,536-byte response budget.
This finite resource-boundary evidence is not a proof of arbitrary future
backend response sizes or new mathematical domains.

## Portable Windows artifact

Application version: `0.14.2`; independent SDK/MCP version: `0.1.0`.
The local package is `solvercraft-0.14.2-windows-x64-portable.zip`:

| Artifact | SHA-256 |
|---|---|
| Portable ZIP | `d206820b177c2c45c78cae5d8242149399b84ec2d73532a11e7f48329c1f3130` |
| `solvercraft.exe` | `5ca7721f63c99237becaef97f2c757db3dfdce272da307f9ddf9fa296c0c3425` |
| `solvercraft-cli.exe` | `2437b11058922252a5c31f8b0df6b5c74fd1b2a2ae6047219fd82944f14dfb50` |
| `solvercraft-mcp.exe` | `519f337c5201edeb3ce19f1bd654f555deefc455e8f5ca964db2f4a7878dca69` |

The ZIP contains 702 entries, 100,124,896 uncompressed bytes and 47,105,838 archive
bytes. CRCs and the exact hashes of all three packaged executables were checked.
The real package passed the pinned launcher's naming, safe-path, checksum,
archive-root executable and Windows x64 PE checks. Nine separate archive guard
controls cover invalid layouts and paths. Existing unrelated checksum rows were
preserved during packaging.

Notices were collected from both locked graphs for the Windows app/CLI/MCP
dependencies: 306 packages and 670 notice files, plus the compiled Craft font
licenses, project attribution and separately licensed numerical data. Nine crates
that omitted license files use checked-in exact upstream-VCS notices with source
URLs and verified hashes; no local environment/path dump is part of the public
inventory. The documented required Craft font inputs were used for host checks
and the application release.

## ArtCraft availability and remaining scope

The [integration guide](artcraft-integration.md) covers the native app, direct
SDK use by other ArtCraft applications, the JSON CLI and local agent setup. The
SDK's portable binding contract is workbook independent; this does not claim
that every separate ArtCraft repository has already installed an adapter.

The [launcher submission](../../integrations/artcraft-launcher/README.md) applies
cleanly to pinned official launcher commit
`ec205366ebe42635e155b9ab246f036052fef406`: 14 exact base and patched Git blobs
were checked. It is a proposed community-app entry, not an accepted global
listing. The official maintainer must approve third-party sources. No upstream
merge, GitHub release publication, publisher signing identity, macOS installer
or executed Linux build is claimed by a source push or this Windows package.

Original mathematical compatibility remains bounded as recorded by the
[96-criterion scope audit](../qa/original-scope-audit.md) and
[remaining reference differences](reference-gaps.md). The previous
[recovery report](recovery-2026-11-02.md) describes the earlier pre-app checkpoint;
its historical source/test records remain unchanged.
