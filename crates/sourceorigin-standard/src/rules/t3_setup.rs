//! `t3-setup`: the build T3 Code starts in each new worktree leaves the
//! machine usable.

use std::collections::BTreeSet;

use super::justfile::Justfile;
use super::t3;
use super::text_file::TextFile;
use super::{Finding, Repository};

/// What the setup recipe's body holds: a default for the jobs, which a set
/// `CARGO_BUILD_JOBS` overrides, and a low priority.
const JOBS: &str = "${CARGO_BUILD_JOBS:-";
const NICE: &str = "nice -n 10";

impl Justfile {
    /// Whether `recipe` runs Cargo, in its body, a dependency, or a recipe
    /// its body runs with `just`.
    fn builds<'a>(&'a self, recipe: &'a str, seen: &mut BTreeSet<&'a str>) -> bool {
        if !seen.insert(recipe) {
            return false;
        }
        let Some(found) = self.recipes.get(recipe) else {
            return false;
        };
        let lines: Vec<String> = found.lines().collect();
        let words: Vec<&str> = lines
            .iter()
            .flat_map(|line| TextFile::tokens(line))
            .map(|(_, word)| word)
            .collect();
        let called: Vec<&'a str> = words
            .windows(2)
            .filter(|pair| pair[0] == "just")
            .filter_map(|pair| self.recipes.get_key_value(pair[1]))
            .map(|(name, _)| name.as_str())
            .collect();
        words.contains(&"cargo")
            || self
                .dependencies(recipe)
                .into_iter()
                .chain(called)
                .any(|other| self.builds(other, seen))
    }
}

impl Repository {
    /// Each script `t3.json` runs on worktree creation that builds without
    /// the low priority and the default jobs in its recipe's body.
    pub(super) fn t3_setup(&self) -> Vec<Finding> {
        let (Some(t3), Some(justfile)) = (&self.t3, &self.justfile) else {
            return Vec::new();
        };
        t3.scripts
            .iter()
            .filter(|script| script.run_on_worktree_create)
            .filter_map(|script| {
                let words: Vec<&str> = script.command.split_whitespace().collect();
                let recipe = match words.as_slice() {
                    ["just", recipe, ..] => *recipe,
                    _ => return None,
                };
                if !justfile.builds(recipe, &mut BTreeSet::new()) {
                    return None;
                }
                let body: Vec<String> = justfile.recipes.get(recipe)?.lines().collect();
                let limited = body.iter().any(|line| line.contains(JOBS))
                    && body.iter().any(|line| line.contains(NICE));
                (!limited).then(|| Finding {
                    path: t3::PATH.into(),
                    line: t3.line(script),
                    message: format!(
                        "`just {recipe}` builds on every core at full priority when a worktree \
                         is created: in its body, `export CARGO_BUILD_JOBS=\"{JOBS}$(( ($(nproc) \
                         + 1) / 2 ))}}\"` and run the build with `{NICE}`"
                    ),
                })
            })
            .collect()
    }
}
