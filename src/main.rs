#![cfg_attr(windows, windows_subsystem = "windows")]

#[path = "windows/logic.rs"]
pub mod logic;

#[cfg(windows)]
mod windows;

#[cfg(windows)]
fn main() {
    if let Err(error) = windows::run() {
        eprintln!("lolPing could not start: {error}");
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("lolPing desktop runtime is supported on Windows 11 x64 only.");
}
