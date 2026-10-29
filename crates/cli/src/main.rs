//! `scribblecap` command line. Commands arrive in roadmap M5-4.

use std::process::ExitCode;

fn main() -> ExitCode {
    match std::env::args().nth(1).as_deref() {
        Some("--version" | "-V") => {
            println!("scribblecap {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        _ => {
            eprintln!(
                "scribblecap {}: no commands yet (document format {})",
                env!("CARGO_PKG_VERSION"),
                scribblecap_core::DOCUMENT_FORMAT
            );
            ExitCode::from(2)
        }
    }
}
