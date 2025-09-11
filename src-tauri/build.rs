fn main() {
    // Skip Tauri build when running tests to avoid libsoup conflicts
    if std::env::var("CARGO_CFG_TEST").is_err() {
        tauri_build::build()
    }
}
