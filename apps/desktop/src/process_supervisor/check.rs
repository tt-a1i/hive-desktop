//! Explicit, non-shipping check entry point; references the production modules.
#![allow(dead_code)]
#[path = "../process_supervisor.rs"]
mod process_supervisor;
#[path = "../protocol.rs"]
mod protocol;
#[path = "../runtime_location.rs"]
mod runtime_location;
#[path = "../transport.rs"]
mod transport;
fn main() {
    eprintln!("Run this target with cargo test; it does not launch the desktop UI.");
}
