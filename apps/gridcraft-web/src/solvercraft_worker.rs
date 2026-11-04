//! Trunk generates the classic-worker loader. All product worker logic is Rust.
#![forbid(unsafe_code)]
#[cfg(target_arch = "wasm32")]
fn main() {
    gridcraft_engine::cmd::solvercraft::browser_worker_main();
}
#[cfg(not(target_arch = "wasm32"))]
fn main() {}
