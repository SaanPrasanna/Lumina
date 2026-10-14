//! Developer automation: `cargo xtask <task>`.
//! Planned tasks: `dist` (build helper + app), `fetch-runtimes` (dev copies of
//! php/nginx into .lumina-dev/), `bump <version>` (sync Cargo/package/tauri versions).

fn main() {
    let task = std::env::args().nth(1);
    match task.as_deref() {
        Some(other) => eprintln!("unknown task: {other}"),
        None => eprintln!("usage: cargo xtask <dist|fetch-runtimes|bump>"),
    }
}
