//! sourceorigin-standard's own development tasks, run with `cargo xtask`:
//! `standard` checks the repository against the standard's rules,
//! `standard rules` lists them, and `check-changed [base]` runs CI's checks
//! for what changed. It uses the crate as piqueld, sourceorigin and
//! DirkEngine do, so this repository runs the checks it ships.

mod ci;

use std::path::Path;
use std::process::ExitCode;

use anyhow::{Context, Result, bail};
use sourceorigin_standard::check_changed::DEFAULT_BASE;
use sourceorigin_standard::rules;

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
        ["standard"] => {
            let violations =
                rules::check(root()).context("check the repository against the standard")?;
            for violation in &violations {
                eprintln!("{violation}");
            }
            if violations.is_empty() {
                Ok(ExitCode::SUCCESS)
            } else {
                eprintln!(
                    "{} violations of the standard; `cargo xtask standard rules` lists the rules",
                    violations.len()
                );
                Ok(ExitCode::FAILURE)
            }
        }
        ["standard", "rules"] => {
            print!("{}", rules::describe());
            Ok(ExitCode::SUCCESS)
        }
        _ => bail!("usage: cargo xtask (standard [rules] | check-changed [base])"),
    }
}
