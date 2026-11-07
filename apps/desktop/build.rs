use std::env;

fn main() {
    // Expanding a GPUI tool group overflows the default Windows main-thread
    // stack in the packaged debug executable. Reserve space for the UI thread;
    // the OS commits pages as they are used.
    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let stack = if env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
            "/STACK:8388608"
        } else {
            "-Wl,--stack,8388608"
        };
        println!("cargo:rustc-link-arg-bin=hive-desktop={stack}");
    }
}
