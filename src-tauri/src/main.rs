// Traceflow — commercial-grade documentation capture tool
// Entry point. All logic lives in the library crate.

#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

fn main() {
    traceflow_lib::run();
}
