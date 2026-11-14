#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if std::env::args().any(|a| a == "--check") {
        #[cfg(windows)]
        unsafe {
            // the release build has no console of its own; borrow the caller's
            extern "system" {
                fn AttachConsole(pid: u32) -> i32;
            }
            AttachConsole(u32::MAX);
        }
        wish_lib::check();
        return;
    }
    wish_lib::run()
}
