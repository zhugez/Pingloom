fn main() {
    if std::env::var("TARGET").is_ok_and(|target| target.contains("windows")) {
        tauri_build::build()
    }
}
