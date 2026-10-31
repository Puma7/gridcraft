# Solvercraft contributor instructions

Read the parent Gridcraft AGENTS.md when this workspace is inside Gridcraft.

Solvercraft is an independent Cargo workspace. Its standard implementation is Rust-only, with no GUI, native library or network requirement at runtime. Keep the core independent of Gridcraft and egui.

Validate public input before allocations and mathematical operations. Non-test code must not panic or use unsafe. Return structured errors. Distinguish termination, feasibility and optimality; do not claim a guarantee merely because a method name or enum exists.

Run cargo fmt --all -- --check, cargo test --workspace --locked and cargo clippy --workspace --all-targets --locked -- -D warnings before proposing changes. Offline is supported when dependencies are cached. Full Gridcraft, visual UI and WASM checks are additional gates for host integration.

When parallelizing, use separate build target directories and explicit module ownership. Coordinate workspace manifests, shared types and lockfile edits. Keep unsupported capabilities explicit and update the reference/acceptance documentation.
