//! `cargo-locked`: recipes, workflows and `t3.json` never let Cargo update
//! `Cargo.lock`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::fix::{Checked, Fix};
use super::t3;
use super::workflows::Workflow;
use super::yaml::{Node, Style};
use super::{Finding, Repository};

/// The justfiles, whose recipe bodies the rule reads, as git pathspecs.
pub(super) const JUSTFILES: [&str; 2] = [":(glob)**/justfile", ":(glob)**/*.just"];

/// The Cargo subcommands that resolve dependencies, so take `--locked`.
/// `cargo xtask` takes it through its alias, which `layout` checks.
const SUBCOMMANDS: [&str; 9] = [
    "build", "check", "clippy", "doc", "llvm-cov", "nextest", "publish", "run", "test",
];

/// The flags that keep `Cargo.lock` as it is.
const LOCKED: [&str; 2] = ["--locked", "--frozen"];

/// A Cargo command without `--locked`: the line its subcommand is on, and
/// where `--locked` goes in it.
pub(super) struct Unlocked<'a> {
    subcommand: &'a str,
    /// The line, from 1.
    line: usize,
    /// The byte of that line after which `--locked` goes.
    insert_at: usize,
}

/// Where the rule reads a line: from which byte, and whether the next line
/// continues its command, as in a folded YAML block.
#[derive(Clone, Copy)]
struct Scan {
    from: usize,
    folds: bool,
}

impl Scan {
    /// A whole line, ending its command unless it ends in `\`.
    const LINE: Self = Self {
        from: 0,
        folds: false,
    };
}

impl Unlocked<'_> {
    /// Each Cargo command in the lines of `text` that `scanned` takes, from
    /// 1, that resolves dependencies without `--locked`. A command runs from
    /// `cargo` to the next `&&`, `||`, `;` or `|`, across lines ending in
    /// `\` or that fold; `#` lines are comments, a heredoc's body and words
    /// inside quotes are text, and `cargo` is only a command as the command
    /// word.
    fn find(text: &str, scanned: impl Fn(usize) -> Option<Scan>) -> Vec<Unlocked<'_>> {
        let mut tokens: Vec<(usize, usize, &str)> = Vec::new();
        let mut found = Vec::new();
        let mut heredoc: Option<&str> = None;
        for (index, line) in text.lines().enumerate() {
            let scan = scanned(index + 1);
            let read = scan.and_then(|scan| line.get(scan.from..));
            if let Some(delimiter) = heredoc {
                if read.is_some_and(|read| read.trim() == delimiter) {
                    heredoc = None;
                }
                continue;
            }
            let (Some(scan), Some(read)) = (scan, read) else {
                found.extend(Self::commands(&tokens));
                tokens.clear();
                continue;
            };
            if read.trim_start().starts_with('#') {
                found.extend(Self::commands(&tokens));
                tokens.clear();
                continue;
            }
            for (start, token) in Self::tokens(read) {
                tokens.push((index + 1, scan.from + start + token.len(), token));
            }
            heredoc = Self::heredoc(read);
            if !scan.folds && !read.trim_end().ends_with('\\') || heredoc.is_some() {
                found.extend(Self::commands(&tokens));
                tokens.clear();
            }
        }
        found.extend(Self::commands(&tokens));
        found
    }

    /// The delimiter of the heredoc `line` opens, such as `EOF` for
    /// `cat <<'EOF'`, whose body is text up to a line of it alone. It
    /// starts with a letter, `_` or a quote, and sits outside an open
    /// `$((`, unlike the shifts in `$((1 << 2))` and `$((1 << n))`.
    fn heredoc(line: &str) -> Option<&str> {
        let at = line.match_indices("<<").map(|(at, _)| at).find(|&at| {
            let before = &line[..at];
            !line[at + 2..].starts_with('<')
                && !before.ends_with('<')
                && before.matches("((").count() <= before.matches("))").count()
        })?;
        let rest = line[at + 2..].trim_start_matches('-').trim_start();
        if !rest.starts_with(|c: char| c.is_ascii_alphabetic() || matches!(c, '_' | '\'' | '"')) {
            return None;
        }
        let rest = rest.trim_start_matches(['\'', '"']);
        let end = rest
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .unwrap_or(rest.len());
        Some(&rest[..end]).filter(|delimiter| !delimiter.is_empty())
    }

    /// The words of `line`, each with the byte it starts at, the first
    /// trimmed of just's `@` and `-`. A `;`, `|` or `&` ends a word and is a
    /// `;` word of its own. A quoted string is one word, which is no
    /// command.
    pub(super) fn tokens(line: &str) -> Vec<(usize, &str)> {
        let mut words = Vec::new();
        let mut start = None;
        let mut quote: Option<char> = None;
        for (at, c) in line.char_indices().chain([(line.len(), ' ')]) {
            let separator = quote.is_none() && matches!(c, ';' | '|' | '&');
            if quote.is_none() && (c.is_whitespace() || separator) {
                if let Some(from) = start.take() {
                    words.push((from, &line[from..at]));
                }
                if separator {
                    words.push((at, ";"));
                }
                continue;
            }
            if start.is_none() {
                start = Some(at);
            }
            match quote {
                Some(open) if c == open => quote = None,
                None if matches!(c, '"' | '\'') => quote = Some(c),
                _ => {}
            }
        }
        words
            .into_iter()
            .enumerate()
            .filter(|(_, (_, word))| !word.starts_with(['"', '\'']))
            .map(|(index, (at, word))| {
                let opened = word.trim_start_matches('(');
                let trimmed = opened
                    .strip_prefix(['@', '-'])
                    .filter(|rest| index == 0 && !rest.starts_with('-'))
                    .unwrap_or(opened);
                let at = at + word.len() - trimmed.len();
                (at, trimmed.trim_end_matches([')', ',']))
            })
            .filter(|(_, word)| !word.is_empty())
            .collect()
    }

    /// Where `command`'s command word is, if it is `cargo`: after any
    /// `NAME=value` assignments, `env`, `exec`, `time` and `nice -n N`, and
    /// the shell keywords a command follows, such as `if`, `do` and `!`.
    fn cargo(command: &[(usize, usize, &str)]) -> Option<usize> {
        let word = |at: usize| command.get(at).map(|&(_, _, word)| word);
        let mut at = 0;
        while let Some(current) = word(at) {
            let assignment = current.split_once('=').is_some_and(|(name, _)| {
                name.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
                    && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
            });
            match current {
                _ if assignment => at += 1,
                "if" | "then" | "elif" | "else" | "do" | "while" | "until" | "!" | "{" => at += 1,
                "env" | "exec" | "time" => {
                    at += 1;
                    while word(at).is_some_and(|word| word.starts_with('-')) {
                        at += 1;
                    }
                }
                "nice" => {
                    at += 1;
                    match word(at) {
                        Some("-n") => at += 2,
                        Some(option) if option.starts_with('-') => at += 1,
                        _ => {}
                    }
                }
                _ => break,
            }
        }
        word(at)
            .is_some_and(|word| word == "cargo" || word.ends_with("/cargo"))
            .then_some(at)
    }

    /// The commands of `tokens` that run a subcommand without `--locked`.
    fn commands<'a>(tokens: &[(usize, usize, &'a str)]) -> Vec<Unlocked<'a>> {
        tokens
            .split(|&(_, _, token)| token == ";")
            .filter_map(|command| {
                let cargo = Self::cargo(command)?;
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

impl Unlocked<'_> {
    /// The finding of each of `unlocked`, in `path`, and for each line
    /// holding any, `fix` given that line's number and the byte offsets
    /// where `--locked` goes, last first.
    fn checked(
        path: &Path,
        unlocked: Vec<Unlocked<'_>>,
        fix: impl Fn(usize, &[usize]) -> Fix,
    ) -> Checked {
        let mut checked = Checked::default();
        let mut by_line: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        for unlocked in unlocked {
            checked.findings.push(Finding {
                path: path.to_owned(),
                line: unlocked.line,
                message: format!(
                    "`cargo {}` without `--locked`: add it, so the command never updates \
                     Cargo.lock",
                    unlocked.subcommand
                ),
            });
            by_line
                .entry(unlocked.line)
                .or_default()
                .push(unlocked.insert_at);
        }
        for (line, mut offsets) in by_line {
            offsets.sort_by_key(|offset| std::cmp::Reverse(*offset));
            checked.fixes.push(fix(line, &offsets));
        }
        checked
    }

    /// `text` with ` --locked` inserted at each of `offsets`, last first.
    fn locked(text: &str, offsets: &[usize]) -> String {
        let mut text = text.to_owned();
        for &offset in offsets {
            text.insert_str(offset.min(text.len()), " --locked");
        }
        text
    }
}

impl Repository {
    /// Each Cargo command that resolves dependencies without `--locked` in
    /// a justfile's recipe bodies (its indented lines), a workflow's or
    /// composite action's `run:` scripts, or a `t3.json` command, fixed by
    /// adding it after the subcommand.
    pub(super) fn cargo_locked(&self) -> Checked {
        let mut checked = Checked::default();
        let mut merge = |more: Checked| {
            checked.findings.extend(more.findings);
            checked.fixes.extend(more.fixes);
        };
        let line_fix = |path: PathBuf, text: String| {
            move |line: usize, offsets: &[usize]| {
                let old = text.lines().nth(line - 1).unwrap_or_default();
                Fix::lines(
                    path.clone(),
                    format!("add `--locked` to line {line}'s Cargo commands"),
                    line..line + 1,
                    Unlocked::locked(old, offsets),
                )
            }
        };
        for justfile in &self.justfiles {
            let lines: Vec<&str> = justfile.text.lines().collect();
            let body = |line: usize| {
                lines[line - 1]
                    .starts_with([' ', '\t'])
                    .then_some(Scan::LINE)
            };
            merge(Unlocked::checked(
                &justfile.path,
                Unlocked::find(&justfile.text, body),
                line_fix(justfile.path.clone(), justfile.text.clone()),
            ));
        }
        for workflow in &self.workflows {
            let mut scans = BTreeMap::new();
            for run in workflow.steps().filter_map(|step| step.get("run")) {
                match run.style() {
                    Some(Style::Literal) => {
                        for (line, _) in run.lines() {
                            scans.insert(line, Scan::LINE);
                        }
                    }
                    Some(Style::Folded) => {
                        let lines = Self::block_lines(&workflow.text, run.line);
                        let last = lines.last().copied();
                        let text: Vec<&str> = workflow.text.lines().collect();
                        for line in lines {
                            let blank =
                                text.get(line - 1).is_none_or(|text| text.trim().is_empty());
                            let folds = Some(line) != last && !blank;
                            scans.insert(line, Scan { from: 0, folds });
                        }
                    }
                    Some(Style::Plain) => {
                        let text = workflow.text.lines().nth(run.line - 1).unwrap_or_default();
                        let from = text
                            .char_indices()
                            .nth(run.column)
                            .map_or(text.len(), |(at, _)| at);
                        scans.insert(run.line, Scan { from, folds: false });
                    }
                    Some(Style::Quoted) => merge(Self::quoted_run(workflow, run, &line_fix)),
                    None => {}
                }
            }
            merge(Unlocked::checked(
                &workflow.path,
                Unlocked::find(&workflow.text, |line| scans.get(&line).copied()),
                line_fix(workflow.path.clone(), workflow.text.clone()),
            ));
        }
        if let Some(t3) = &self.t3 {
            for script in &t3.scripts {
                let line = t3.line(script);
                let unlocked: Vec<Unlocked<'_>> =
                    Unlocked::find(&script.command, |_| Some(Scan::LINE))
                        .into_iter()
                        .map(|unlocked| Unlocked { line, ..unlocked })
                        .collect();
                merge(Unlocked::checked(
                    Path::new(t3::PATH),
                    unlocked,
                    |line, offsets| {
                        let json =
                            |command: &str| serde_json::to_string(command).unwrap_or_default();
                        let locked = Unlocked::locked(&script.command, offsets);
                        Fix::lines(
                            t3::PATH,
                            format!("add `--locked` to line {line}'s Cargo commands"),
                            line..line + 1,
                            t3.text_of(line)
                                .replacen(&json(&script.command), &json(&locked), 1),
                        )
                    },
                ));
            }
        }
        checked
    }

    /// The lines of `text`, from 1, of the block scalar whose text starts on
    /// line `first`: those up to the first that is less indented and not
    /// blank, which a folded block's value no longer shows.
    fn block_lines(text: &str, first: usize) -> Vec<usize> {
        let indent = |line: &str| line.len() - line.trim_start().len();
        let lines: Vec<&str> = text.lines().collect();
        let Some(start) = lines.get(first - 1).map(|line| indent(line)) else {
            return Vec::new();
        };
        let mut block: Vec<usize> = (first..=lines.len())
            .take_while(|&line| {
                let text = lines[line - 1];
                text.trim().is_empty() || indent(text) >= start
            })
            .collect();
        while block
            .last()
            .is_some_and(|&line| lines[line - 1].trim().is_empty())
        {
            block.pop();
        }
        block
    }

    /// The Cargo commands without `--locked` of `run`, a quoted scalar, read
    /// from its value; fixed in place when the line holds the value as
    /// written, without escapes.
    fn quoted_run<F: Fn(usize, &[usize]) -> Fix>(
        workflow: &Workflow,
        run: &Node,
        line_fix: &impl Fn(PathBuf, String) -> F,
    ) -> Checked {
        let Some(value) = run.as_str() else {
            return Checked::default();
        };
        let text = workflow.text.lines().nth(run.line - 1).unwrap_or_default();
        let base = text
            .char_indices()
            .nth(run.column + 1)
            .map(|(at, _)| at)
            .filter(|&at| text[at..].starts_with(value));
        let unlocked = Unlocked::find(value, |line| (line == 1).then_some(Scan::LINE))
            .into_iter()
            .map(|unlocked| Unlocked {
                line: run.line,
                insert_at: base.map_or(0, |base| base + unlocked.insert_at),
                ..unlocked
            })
            .collect();
        let mut checked = Unlocked::checked(
            &workflow.path,
            unlocked,
            line_fix(workflow.path.clone(), workflow.text.clone()),
        );
        if base.is_none() {
            checked.fixes.clear();
        }
        checked
    }
}
