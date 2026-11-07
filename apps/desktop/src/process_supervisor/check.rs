//! Explicit, non-shipping check entry point; references the production modules.
#![allow(dead_code)]
#[path = "../process_supervisor.rs"]
mod process_supervisor;
#[path = "../protocol.rs"]
mod protocol;
#[path = "../runtime_location.rs"]
mod runtime_location;
#[cfg(target_os = "macos")]
#[path = "../shell_owner.rs"]
mod shell_owner;
#[path = "../transport.rs"]
mod transport;
fn main() {
    #[cfg(target_os = "macos")]
    shell_owner::run_if_requested();
    eprintln!("Run this target with cargo test; it does not launch the desktop UI.");
}
