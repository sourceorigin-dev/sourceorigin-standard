//! `budget`: each crate's exceptions stay within a committed ceiling.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;
use toml::Spanned;

use super::comments::Comment;
use super::fix::{Checked, Fix};
use super::level::Level;
use super::toml_file::TomlFile;
use super::{Error, Finding, Repository};

/// Where the budget lives, from the repository's root.
pub(super) const PATH: &str = "budget.toml";

/// One table of `budget.toml`: each crate's ceiling for one kind of
/// exception.
#[derive(Debug, Default)]
pub(super) struct Ceilings(BTreeMap<String, Ceiling>);

/// A crate's ceiling, with its line.
#[derive(Clone, Copy, Debug)]
struct Ceiling {
    count: usize,
    line: usize,
}

impl Ceiling {
    /// None, for a crate a table doesn't list.
    const NONE: Self = Self { count: 0, line: 1 };
}

/// `budget.toml`: per crate, the most exceptions of each kind it may hold.
/// Missing, it allows none.
#[derive(Debug, Default)]
pub(super) struct Budget {
    /// `[expect]`: `#[expect]` attributes.
    expect: Ceilings,
    /// `[invariant]`: `// INVARIANT:` comments.
    invariant: Ceilings,
}

/// `budget.toml` as written, before its spans become lines.
#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct File {
    expect: BTreeMap<Spanned<String>, usize>,
    invariant: BTreeMap<Spanned<String>, usize>,
}

impl Budget {
    /// Loads the budget under `root`.
    pub(super) fn load(root: &Path) -> Result<Self, Error> {
        let Some(file) = TomlFile::<File>::load(root, PATH)? else {
            return Ok(Self::default());
        };
        Ok(Self {
            expect: Ceilings::of(&file, &file.value.expect),
            invariant: Ceilings::of(&file, &file.value.invariant),
        })
    }
}

impl Ceilings {
    fn of(file: &TomlFile<File>, table: &BTreeMap<Spanned<String>, usize>) -> Self {
        Self(
            table
                .iter()
                .map(|(name, &count)| {
                    let line = file.line(name);
                    (name.get_ref().clone(), Ceiling { count, line })
                })
                .collect(),
        )
    }

    /// Each crate of `counts` over its ceiling in the `[table]` of
    /// `what`, and each name the table lists that is no crate.
    fn findings(&self, table: &str, what: &str, counts: &BTreeMap<&str, usize>) -> Vec<Finding> {
        let over = counts.iter().filter_map(|(&name, &count)| {
            let ceiling = self.0.get(name).copied().unwrap_or(Ceiling::NONE);
            (count > ceiling.count).then(|| Finding {
                path: PATH.into(),
                line: ceiling.line,
                message: format!(
                    "`{name}` has {count} {what}, over its ceiling of {}: remove one, or raise \
                     `{name}` under `[{table}]` in {PATH}, which the owner reviews",
                    ceiling.count
                ),
            })
        });
        let unknown = self
            .0
            .iter()
            .filter(|(name, _)| !counts.contains_key(name.as_str()))
            .map(|(name, ceiling)| Finding {
                path: PATH.into(),
                line: ceiling.line,
                message: format!("`{name}` is no workspace crate: remove it from `[{table}]`"),
            });
        over.chain(unknown).collect()
    }

    /// The fixes of the `[table]`: lower each ceiling to its crate's count
    /// in `counts`, and remove each name that is no crate.
    fn fixes(&self, table: &'static str, counts: &BTreeMap<&str, usize>) -> Vec<Fix> {
        self.0
            .iter()
            .filter_map(|(name, ceiling)| {
                let key = name.clone();
                match counts.get(name.as_str()) {
                    Some(&count) if count < ceiling.count => Some(Fix::new(
                        PATH,
                        format!(
                            "lower `{name}` under `[{table}]` from {} to {count}",
                            ceiling.count
                        ),
                        move |document| {
                            if let Some(item) = document[table]
                                .as_table_like_mut()
                                .and_then(|entries| entries.get_mut(&key))
                            {
                                Fix::set(item, i64::try_from(count).unwrap_or(i64::MAX));
                            }
                        },
                    )),
                    Some(_) => None,
                    None => Some(Fix::new(
                        PATH,
                        format!("remove `{name}`, no workspace crate, from `[{table}]`"),
                        move |document| Fix::remove(&mut document[table], &key),
                    )),
                }
            })
            .collect()
    }
}

impl Repository {
    /// Each crate's count of `kind` in its Rust files, every workspace crate
    /// included.
    fn per_crate(&self, count: impl Fn(&super::source::Source) -> usize) -> BTreeMap<&str, usize> {
        let mut counts: BTreeMap<&str, usize> = self
            .metadata
            .packages
            .iter()
            .map(|package| (package.name.as_str(), 0))
            .collect();
        for source in &self.sources {
            if let Some(package) = self.metadata.package_of(&source.path.to_string_lossy()) {
                *counts.entry(package.name.as_str()).or_default() += count(source);
            }
        }
        counts
    }

    /// Each crate over its ceiling of `#[expect]` attributes or of
    /// `// INVARIANT:` comments, and each budgeted name that is no crate.
    pub(super) fn budget(&self) -> Checked {
        let expects = self.per_crate(|source| {
            source
                .lints
                .iter()
                .filter(|lint| lint.level == Level::Expect)
                .count()
        });
        let invariants =
            self.per_crate(|source| source.comments.iter().map(Comment::invariants).sum());
        let mut findings =
            self.budget
                .expect
                .findings("expect", "`#[expect]` attributes", &expects);
        findings.extend(self.budget.invariant.findings(
            "invariant",
            "`// INVARIANT:` comments",
            &invariants,
        ));
        let mut fixes = self.budget.expect.fixes("expect", &expects);
        fixes.extend(self.budget.invariant.fixes("invariant", &invariants));
        Checked { findings, fixes }
    }
}
