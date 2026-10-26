#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

#[cfg(target_os = "windows")]
fn main() {
    if let Err(error) = koett_engine::windows::run() {
        koett_engine::windows::show_fatal_error(&error);
    }
}

#[cfg(target_os = "linux")]
fn main() {
    if let Err(error) = koett_engine::linux::run() {
        koett_engine::linux::show_fatal_error(&error);
    }
}

#[cfg(not(any(target_os = "windows", target_os = "linux")))]
fn main() {
    eprintln!("The Koett desktop shell supports Windows and Linux.");
}
