//! `cdl` コマンド（`check` / `eval`）。
#![forbid(unsafe_code)]

use std::io::{stderr, stdout};
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = cdl_cli::run(&args, &mut stdout().lock(), &mut stderr().lock());
    ExitCode::from(code)
}
