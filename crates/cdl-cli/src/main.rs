//! `cdl` コマンド（`check` / `eval`）。
#![forbid(unsafe_code)]

use std::process::ExitCode;

fn main() -> ExitCode {
    eprintln!("使い方: cdl <check|eval> <file>（未実装）");
    ExitCode::from(2)
}
