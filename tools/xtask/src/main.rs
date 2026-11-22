//! sourceorigin-standard's own development tasks, run with `cargo xtask`:
//! `standard` checks the repository against the standard's rules (`--fix`
//! first makes their mechanical fixes), `standard report` prints JSON for the
//! cleanup pass, `standard rules` lists them, `readme` writes that list into
//! the crate's README, and `check-changed [base]` runs CI's checks
//! for what changed. It uses the crate as piqueld, sourceorigin and
//! DirkEngine do, so this repository runs the checks it ships.

mod ci;
mod readme;

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
        ["standard", rest @ ..] if matches!(rest, [] | ["--fix"]) => {
            if rest == ["--fix"] {
                for fix in rules::fix(root()).context("fix the repository")? {
                    println!("fixed {fix}");
                }
            }
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
        ["standard", "report"] => {
            let report = rules::report(root()).context("report on the repository")?;
            println!("{}", report.to_json().context("print the report as JSON")?);
            Ok(ExitCode::SUCCESS)
        }
        ["readme"] => {
            readme::write().context("write the README's rules")?;
            Ok(ExitCode::SUCCESS)
        }
        ["standard", "rules"] => {
            print!("{}", rules::describe());
            Ok(ExitCode::SUCCESS)
        }
        _ => bail!(
            "usage: cargo xtask (standard [--fix | report | rules] | check-changed [base] | readme)"
        ),
    }
}
