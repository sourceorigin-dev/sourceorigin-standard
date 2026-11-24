//! `budget`: each crate's exceptions number exactly what a committed file
//! says, so a new one always raises a number there.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use toml::Spanned;

use super::comments::Comment;
use super::config_syntax::Unreadable;
use super::fix::{Checked, Fix};
use super::level::Level;
use super::modules::Modules;
use super::source::{Source, Unparsed};
use super::toml_file::TomlFile;
use super::{Error, Finding, Repository};
use crate::metadata::{Metadata, MetadataError, TargetKind};
use crate::workspace::{CommandError, Workspace};

/// Where the budget lives, from the repository's root.
pub(super) const PATH: &str = "budget.toml";

/// One table of `budget.toml`: each crate's, or manifest's, ceiling for one
/// kind of exception.
#[derive(Debug, Default)]
pub(super) struct Ceilings(BTreeMap<String, Ceiling>);

/// A crate's ceiling, with its line.
#[derive(Clone, Copy, Debug)]
struct Ceiling {
    count: usize,
    line: usize,
}

/// `budget.toml`: per crate, the most exceptions of each kind it may hold,
/// and per manifest, the most lints it may allow. Missing, it allows none.
#[derive(Debug, Default)]
pub(super) struct Budget {
    /// `[expect]`: `#[expect]` lints.
    expect: Ceilings,
    /// `[invariant]`: `// INVARIANT:` comments.
    invariant: Ceilings,
    /// `[allow]`: lints a manifest's `[lints]` or `[workspace.lints]` sets
    /// to `allow`, by the manifest's path.
    allow: Ceilings,
}

/// `budget.toml` as written, before its spans become lines.
#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct File {
    expect: BTreeMap<Spanned<String>, usize>,
    invariant: BTreeMap<Spanned<String>, usize>,
    allow: BTreeMap<Spanned<String>, usize>,
}

/// Another workspace the repository holds, such as one on its own
/// toolchain: its crates have budgets too.
#[derive(Debug)]
pub(super) struct OtherWorkspace {
    metadata: Metadata,
    modules: Modules,
}

/// What one table counts: its kind of exception, and what its keys name.
struct Table {
    name: &'static str,
    what: &'static str,
    keys: &'static str,
}

const EXPECT: Table = Table {
    name: "expect",
    what: "`#[expect]` lints",
    keys: "crate",
};
const INVARIANT: Table = Table {
    name: "invariant",
    what: "`// INVARIANT:` lines",
    keys: "crate",
};
const ALLOW: Table = Table {
    name: "allow",
    what: "lints set to `allow`",
    keys: "manifest",
};

/// A count of one key, and where its finding goes when the table doesn't
/// list it: the crate's manifest, or the manifest itself.
struct Count {
    count: usize,
    home: PathBuf,
}

impl Budget {
    /// Loads the budget under `root`.
    pub(super) fn load(root: &Path, unreadable: &mut Unreadable) -> Result<Self, Error> {
        let Some(file) = TomlFile::<File>::load(root, PATH, unreadable)? else {
            return Ok(Self::default());
        };
        Ok(Self {
            expect: Ceilings::of(&file, &file.value.expect),
            invariant: Ceilings::of(&file, &file.value.invariant),
            allow: Ceilings::of(&file, &file.value.allow),
        })
    }

    /// The crate or manifest whose entry is at `line` of `budget.toml`, to
    /// which a deviation scoped to crates applies.
    pub(super) fn key_at(&self, line: usize) -> Option<&str> {
        [&self.expect, &self.invariant, &self.allow]
            .into_iter()
            .flat_map(|ceilings| &ceilings.0)
            .find(|(_, ceiling)| ceiling.line == line)
            .map(|(name, _)| name.as_str())
    }
}

impl OtherWorkspace {
    /// Every other workspace under `root`: each manifest git tracks, outside
    /// the crates of `metadata`, with a `[workspace]` table. One `cargo
    /// metadata` can't read is a violation of `config-syntax`.
    pub(super) fn load_all(
        root: &Path,
        metadata: &Metadata,
        sources: &[Source],
        unreadable: &mut Unreadable,
    ) -> Result<Vec<Self>, Error> {
        let workspace = Workspace::new(root);
        let mut others = Vec::new();
        for path in workspace.files(&[":(glob)**/Cargo.toml"])? {
            if path == super::manifest::ROOT || metadata.package_of(&path).is_some() {
                continue;
            }
            let Some(manifest) = TomlFile::<toml::Table>::load(root, &path, unreadable)? else {
                continue;
            };
            if !manifest.value.contains_key("workspace") {
                continue;
            }
            match metadata.other(workspace, &path) {
                Ok(other) => others.push(Self {
                    modules: Modules::of(&other, sources),
                    metadata: other,
                }),
                Err(MetadataError::Command(CommandError::Failed { stderr, .. })) => {
                    let unparsed = Unparsed::cargo(root, &stderr)?;
                    unreadable.record(unparsed.path, unparsed.line, unparsed.message);
                }
                Err(error) => return Err(error.into()),
            }
        }
        Ok(others)
    }
}

/// A UI case is told apart by the crate it depends on, which Cargo names
/// by a string, as the rules module's names are.
impl Metadata {
    /// Whether `path`, from the workspace's root, is a Dylint library's UI
    /// case: the root of an example of a crate depending on
    /// `dylint_testing`. Such a case is a test fixture, whose `#[expect]`s are
    /// what it tests.
    fn is_ui_case(&self, path: &Path) -> bool {
        let file = self.workspace_root.join(path);
        self.packages
            .iter()
            .filter(|package| {
                package
                    .dependencies
                    .iter()
                    .any(|dependency| dependency.name == "dylint_testing")
            })
            .flat_map(|package| &package.targets)
            .filter(|target| target.kind.contains(&TargetKind::Example))
            .any(|target| target.src_path == file)
    }
}

/// The lints `manifest` sets to `allow`, in `[lints]` and
/// `[workspace.lints]`, in any of TOML's forms.
fn allows(manifest: &toml::Table) -> usize {
    let level = |setting: &toml::Value| {
        setting
            .as_str()
            .or_else(|| setting.get("level").and_then(toml::Value::as_str))
            .map(str::to_owned)
    };
    [
        manifest.get("lints"),
        manifest
            .get("workspace")
            .and_then(|workspace| workspace.get("lints")),
    ]
    .into_iter()
    .flatten()
    .filter_map(toml::Value::as_table)
    .flat_map(toml::Table::values)
    .filter_map(toml::Value::as_table)
    .flat_map(toml::Table::values)
    .filter(|setting| level(setting).as_deref() == Some("allow"))
    .count()
}

/// Each manifest of the repository with the lints it allows: the root's,
/// each crate's of `metadata`, and each of `others`.
pub(super) fn manifest_allows(
    root: &Path,
    metadata: &Metadata,
    others: &[OtherWorkspace],
    unreadable: &mut Unreadable,
) -> Result<BTreeMap<String, usize>, Error> {
    let mut paths = vec![PathBuf::from(super::manifest::ROOT)];
    for metadata in std::iter::once(metadata).chain(others.iter().map(|other| &other.metadata)) {
        for package in &metadata.packages {
            let path = &package.manifest_path;
            paths.push(
                path.strip_prefix(&metadata.workspace_root)
                    .unwrap_or(path)
                    .to_owned(),
            );
        }
    }
    paths.sort();
    paths.dedup();
    let mut counts = BTreeMap::new();
    for path in paths {
        if let Some(manifest) = TomlFile::<toml::Table>::load(root, &path, unreadable)? {
            counts.insert(path.to_string_lossy().into_owned(), allows(&manifest.value));
        }
    }
    Ok(counts)
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

    /// Each key of `counts` whose count isn't its budget in `table`, and
    /// each key the table lists that `counts` doesn't hold. A key the table
    /// doesn't list is reported at its home, as it has no line here.
    fn findings(&self, table: &Table, counts: &BTreeMap<String, Count>) -> Vec<Finding> {
        let Table {
            name: table,
            what,
            keys,
        } = table;
        let off = counts.iter().filter_map(|(name, count)| {
            let ceiling = self.0.get(name);
            let budget = ceiling.map_or(0, |ceiling| ceiling.count);
            let count = count.count;
            let message = match count.cmp(&budget) {
                std::cmp::Ordering::Equal => return None,
                std::cmp::Ordering::Greater => format!(
                    "`{name}` has {count} {what}, over its budget of {budget}: remove one, or \
                     raise `{name}` under `[{table}]` in {PATH}, which the owner reviews"
                ),
                std::cmp::Ordering::Less => format!(
                    "`{name}` has {count} {what}, under its budget of {budget}: lower `{name}` \
                     under `[{table}]` in {PATH} to {count}, as `--fix` does"
                ),
            };
            let (path, line) = ceiling.map_or_else(
                || (counts[name].home.clone(), 1),
                |ceiling| (PathBuf::from(PATH), ceiling.line),
            );
            Some(Finding {
                path,
                line,
                message,
            })
        });
        let unknown = self
            .0
            .iter()
            .filter(|(name, _)| !counts.contains_key(name.as_str()))
            .map(|(name, ceiling)| Finding {
                path: PATH.into(),
                line: ceiling.line,
                message: format!(
                    "`{name}` is no {keys} of the repository: remove it from `[{table}]`"
                ),
            });
        off.chain(unknown).collect()
    }

    /// The fixes of `table`: lower each ceiling to its key's count in
    /// `counts`, and remove each key that `counts` doesn't hold.
    fn fixes(&self, table: &Table, counts: &BTreeMap<String, Count>) -> Vec<Fix> {
        let Table {
            name: table, keys, ..
        } = *table;
        self.0
            .iter()
            .filter_map(|(name, ceiling)| {
                let key = name.clone();
                match counts.get(name.as_str()) {
                    Some(&Count { count, .. }) if count < ceiling.count => Some(Fix::toml(
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
                    None => Some(Fix::toml(
                        PATH,
                        format!("remove `{name}`, no {keys} of the repository, from `[{table}]`"),
                        move |document| Fix::remove(&mut document[table], &key),
                    )),
                }
            })
            .collect()
    }
}

impl Repository {
    /// Each crate's count of `count` over the Rust files its targets reach,
    /// every crate of the repository included, the other workspaces' too,
    /// but for a Dylint library's UI cases, which are test fixtures.
    fn per_crate(&self, count: impl Fn(&Source) -> usize) -> BTreeMap<String, Count> {
        let workspaces = std::iter::once((&self.metadata, &self.modules)).chain(
            self.other_workspaces
                .iter()
                .map(|other| (&other.metadata, &other.modules)),
        );
        let mut counts = BTreeMap::new();
        for (metadata, modules) in workspaces {
            for package in &metadata.packages {
                let manifest = &package.manifest_path;
                counts.entry(package.name.clone()).or_insert(Count {
                    count: 0,
                    home: manifest
                        .strip_prefix(&metadata.workspace_root)
                        .unwrap_or(manifest)
                        .to_owned(),
                });
            }
            for source in &self.sources {
                if modules.of_file(&source.path).is_empty() || metadata.is_ui_case(&source.path) {
                    continue;
                }
                if let Some(package) = metadata.package_of(&source.path.to_string_lossy())
                    && let Some(entry) = counts.get_mut(&package.name)
                {
                    entry.count += count(source);
                }
            }
        }
        counts
    }

    /// Each crate whose `#[expect]` lints or `// INVARIANT:` lines, and each
    /// manifest whose `allow`ed lints, don't number their budget, and each
    /// budgeted name that is none of them.
    pub(super) fn budget(&self) -> Checked {
        let expects = self.per_crate(|source| {
            source
                .lints
                .iter()
                .filter(|lint| lint.level == Level::Expect)
                .map(|lint| lint.lints.len())
                .sum()
        });
        let invariants =
            self.per_crate(|source| source.comments.iter().map(Comment::invariants).sum());
        let allows = self
            .manifest_allows
            .iter()
            .map(|(path, &count)| {
                let home = PathBuf::from(path);
                (path.clone(), Count { count, home })
            })
            .collect();
        let tables = [
            (&self.budget.expect, EXPECT, expects),
            (&self.budget.invariant, INVARIANT, invariants),
            (&self.budget.allow, ALLOW, allows),
        ];
        let mut checked = Checked::default();
        for (ceilings, table, counts) in &tables {
            checked.findings.extend(ceilings.findings(table, counts));
            checked.fixes.extend(ceilings.fixes(table, counts));
        }
        checked
    }
}
