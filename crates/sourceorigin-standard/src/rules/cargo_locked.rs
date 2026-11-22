//! `cargo-locked`: recipes, workflows and `t3.json` never let Cargo update
//! `Cargo.lock`.

use std::collections::BTreeMap;

use super::fix::{Checked, Fix};
use super::text_file::TextFile;
use super::{Finding, Repository};

/// The files whose commands the rule reads, as git pathspecs.
pub(super) const FILES: [&str; 5] = [
    ":(glob)**/justfile",
    ":(glob)**/*.just",
    ":(glob).github/**/*.yml",
    ":(glob).github/**/*.yaml",
    "t3.json",
];

/// The Cargo subcommands that resolve dependencies, so take `--locked`.
/// `cargo xtask` takes it through its alias, which `layout` checks.
const SUBCOMMANDS: [&str; 9] = [
    "build", "check", "clippy", "doc", "llvm-cov", "nextest", "publish", "run", "test",
];

/// The flags that keep `Cargo.lock` as it is.
const LOCKED: [&str; 2] = ["--locked", "--frozen"];

/// A Cargo command without `--locked`: the line its subcommand is on, and
/// where `--locked` goes in it.
struct Unlocked<'a> {
    subcommand: &'a str,
    /// The line, from 1.
    line: usize,
    /// The byte of that line after which `--locked` goes.
    insert_at: usize,
}

impl TextFile {
    /// Each Cargo command of the file that resolves dependencies without
    /// `--locked`. A command runs from `cargo` to the next `&&`, `||`, `;`
    /// or `|`, across lines ending in `\`; `#` lines are comments.
    fn unlocked(&self) -> Vec<Unlocked<'_>> {
        let mut tokens: Vec<(usize, usize, &str)> = Vec::new();
        let mut found = Vec::new();
        for (index, line) in self.text.lines().enumerate() {
            if line.trim_start().starts_with('#') {
                continue;
            }
            let continued = line.trim_end().ends_with('\\');
            for (start, token) in Self::tokens(line) {
                tokens.push((index + 1, start + token.len(), token));
            }
            if !continued {
                found.extend(Self::commands(&tokens));
                tokens.clear();
            }
        }
        found.extend(Self::commands(&tokens));
        found
    }

    /// The words of `line`, each with the byte it starts at, trimmed of
    /// quotes and of just's `@` and `-`. A `;`, `|` or `&` ends a word and
    /// is a `;` word of its own.
    fn tokens(line: &str) -> Vec<(usize, &str)> {
        let mut words = Vec::new();
        let mut start = None;
        for (at, c) in line.char_indices().chain([(line.len(), ' ')]) {
            let separator = matches!(c, ';' | '|' | '&');
            if c.is_whitespace() || separator {
                if let Some(from) = start.take() {
                    words.push((from, &line[from..at]));
                }
                if separator {
                    words.push((at, ";"));
                }
            } else if start.is_none() {
                start = Some(at);
            }
        }
        words
            .into_iter()
            .map(|(at, word)| {
                let quoted = word.trim_start_matches(['"', '\'', '(']);
                let trimmed = quoted
                    .strip_prefix(['@', '-'])
                    .filter(|rest| !rest.starts_with('-'))
                    .unwrap_or(quoted);
                let at = at + word.len() - trimmed.len();
                (at, trimmed.trim_end_matches(['"', '\'', ')', ',']))
            })
            .filter(|(_, word)| !word.is_empty())
            .collect()
    }

    /// The commands of `tokens` that run a subcommand without `--locked`.
    fn commands<'a>(tokens: &[(usize, usize, &'a str)]) -> Vec<Unlocked<'a>> {
        tokens
            .split(|&(_, _, token)| token == ";")
            .filter_map(|command| {
                let cargo = command
                    .iter()
                    .position(|&(_, _, token)| token == "cargo" || token.ends_with("/cargo"))?;
                let mut rest = command[cargo + 1..]
                    .iter()
                    .skip_while(|(_, _, token)| token.starts_with('+'));
                let &(line, end, subcommand) = rest.next()?;
                if !SUBCOMMANDS.contains(&subcommand)
                    || command.iter().any(|(_, _, token)| LOCKED.contains(token))
                {
                    return None;
                }
                let (line, insert_at) = match rest.next() {
                    Some(&(run_line, run_end, "run")) if subcommand == "nextest" => {
                        (run_line, run_end)
                    }
                    _ => (line, end),
                };
                Some(Unlocked {
                    subcommand,
                    line,
                    insert_at,
                })
            })
            .collect()
    }
}

impl Repository {
    /// Each Cargo command in a justfile, workflow or `t3.json` that
    /// resolves dependencies without `--locked`, fixed by adding it after
    /// the subcommand.
    pub(super) fn cargo_locked(&self) -> Checked {
        let mut checked = Checked::default();
        for file in &self.scripts {
            let mut by_line: BTreeMap<usize, Vec<Unlocked<'_>>> = BTreeMap::new();
            for unlocked in file.unlocked() {
                checked.findings.push(Finding {
                    path: file.path.clone(),
                    line: unlocked.line,
                    message: format!(
                        "`cargo {}` without `--locked`: add it, so the command never updates \
                         Cargo.lock",
                        unlocked.subcommand
                    ),
                });
                by_line.entry(unlocked.line).or_default().push(unlocked);
            }
            for (line, mut unlocked) in by_line {
                let mut text = file
                    .text
                    .lines()
                    .nth(line - 1)
                    .unwrap_or_default()
                    .to_owned();
                unlocked.sort_by_key(|unlocked| std::cmp::Reverse(unlocked.insert_at));
                for unlocked in &unlocked {
                    text.insert_str(unlocked.insert_at.min(text.len()), " --locked");
                }
                checked.fixes.push(Fix::line(
                    file.path.clone(),
                    format!("add `--locked` to line {line}'s Cargo commands"),
                    line,
                    text,
                ));
            }
        }
        checked
    }
}
