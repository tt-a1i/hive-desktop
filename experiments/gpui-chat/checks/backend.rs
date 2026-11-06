#[path = "../src/backend.rs"]
mod backend;
fn main() {
    let endpoint =
        std::env::var("HIVE_PROBE_URL").expect("set HIVE_PROBE_URL to an isolated Hive backend");
    let response = backend::probe_local(&endpoint);
    println!("{response}");
    assert!(
        response.starts_with("session 200 → HTTP/1.1 200 ")
            || response.starts_with("session 200 → HTTP/1.0 200 ")
    );
    assert!(
        response.ends_with("[]"),
        "isolated backend must contain no workspaces"
    );
}
