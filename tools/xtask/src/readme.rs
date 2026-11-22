//! The crate README's list of rules, generated from the docs of `Rule`, so
//! the README is the written standard. `just readme` writes it, and
//! `the_readme_lists_the_rules_as_documented` fails while it is stale.

use std::path::PathBuf;

use anyhow::{Context, Result};
use sourceorigin_standard::rules;

/// The line before the list.
const START: &str = "<!-- rules:start: generated from the docs of `Rule` by `just readme` -->";

/// The line after the list.
const END: &str = "<!-- rules:end -->";

/// The crate's README.
fn path() -> PathBuf {
    crate::root().join("crates/sourceorigin-standard/README.md")
}

/// `readme` with the list between its markers replaced by the rules'.
fn render(readme: &str) -> Result<String> {
    let (before, rest) = readme
        .split_once(START)
        .with_context(|| format!("find `{START}` in the README"))?;
    let (_, after) = rest
        .split_once(END)
        .with_context(|| format!("find `{END}` in the README"))?;
    Ok(format!(
        "{before}{START}\n{}{END}{after}",
        rules::describe()
    ))
}

/// Writes the rules' list into the README.
pub fn write() -> Result<()> {
    let path = path();
    let readme =
        std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    std::fs::write(&path, render(&readme)?).with_context(|| format!("write {}", path.display()))
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_readme_lists_the_rules_as_documented() {
        let readme = std::fs::read_to_string(super::path()).expect("the README is read");
        let rendered = super::render(&readme).expect("the README has its markers");
        assert_eq!(
            rendered, readme,
            "the README's rules are stale: run `just readme`"
        );
    }
}
