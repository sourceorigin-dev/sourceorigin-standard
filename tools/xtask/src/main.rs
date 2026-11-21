//! sourceorigin-xtask's own development tasks, run with `cargo xtask`:
//! `check-changed [base]` and `test-waits`. It uses the crate as piqueld,
//! sourceorigin and DirkEngine do, so this repository runs the checks it
//! ships.

mod ci;

use std::path::Path;
use std::process::ExitCode;

use anyhow::{Context, Result, bail};
use sourceorigin_xtask::check_changed::DEFAULT_BASE;

/// The workspace's root, two directories above this crate's.
fn root() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
}

fn main() -> Result<ExitCode> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["check-changed", rest @ ..] if rest.len() <= 1 => {
            let base = rest.first().copied().unwrap_or(DEFAULT_BASE);
            ci::CI
                .run(root(), base)
                .context("check what changed since the merge base")
        }
        ["test-waits"] => {
            sourceorigin_xtask::test_waits::run(root())
                .context("check that tests wait for events")?;
            println!("No test waits on the clock.");
            Ok(ExitCode::SUCCESS)
        }
        _ => bail!("usage: cargo xtask check-changed [base] | cargo xtask test-waits"),
    }
}
