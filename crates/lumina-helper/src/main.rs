//! Elevated helper. The GUI never runs as admin; it launches this binary with
//! elevation for a single, validated operation (hosts edit, CA install, system PATH)
//! and exits. Keep its surface minimal and audited.

fn main() {
    // TODO: parse one sub-command (e.g. `hosts-sync <file>`), validate it, execute, exit.
}
