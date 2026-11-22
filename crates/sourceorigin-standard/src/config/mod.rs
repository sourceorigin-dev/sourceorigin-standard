//! `standard.toml`: how a repository applies the standard.

#[cfg(test)]
mod tests;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use globset::{GlobBuilder, GlobMatcher};
use serde::Deserialize;
use toml::Spanned;

use crate::rules::Rule;

/// Where the configuration lives, from the repository's root.
pub(crate) const PATH: &str = "standard.toml";

/// Why `standard.toml` couldn't be loaded.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// It exists but couldn't be read.
    #[error("read {}", path.display())]
    Read {
        /// The file.
        path: PathBuf,
        /// Why it couldn't be read.
        #[source]
        source: std::io::Error,
    },
    /// It isn't valid TOML, or holds a key or value the standard doesn't
    /// know, such as a misspelled rule.
    #[error("{PATH}: {message}")]
    Parse {
        /// The parser's message, naming the bad key and where it is.
        message: String,
    },
    /// A deviation gives no reason.
    #[error("{PATH}:{line}: the deviation from `{rule}` needs a reason")]
    NoReason {
        /// The rule's name.
        rule: &'static str,
        /// The deviation's line, from 1.
        line: usize,
    },
    /// A scoped deviation names neither `paths` nor `crates`.
    #[error(
        "{PATH}:{line}: the deviation from `{rule}` needs `paths` or `crates`, or only a reason \
         to apply everywhere"
    )]
    NoScope {
        /// The rule's name.
        rule: &'static str,
        /// The deviation's line, from 1.
        line: usize,
    },
    /// A deviation's path glob is invalid.
    #[error("{PATH}:{line}: the deviation's path")]
    Glob {
        /// The glob's line, from 1.
        line: usize,
        /// Which glob, and why.
        #[source]
        source: globset::Error,
    },
}

/// A repository's `standard.toml`. Every key is optional; a missing file is
/// the defaults.
#[derive(Debug, Default)]
pub(crate) struct Config {
    /// The rules the repository departs from.
    pub(crate) deviations: Vec<Deviation>,
    /// Settings of `check-changed`.
    pub(crate) check_changed: CheckChanged,
}

/// A rule the repository departs from on purpose, everywhere or only in
/// the files and crates its scope names.
#[derive(Debug)]
pub(crate) struct Deviation {
    pub(crate) rule: Rule,
    /// Why, which the file must say.
    pub(crate) reason: String,
    /// Its line in `standard.toml`, from 1.
    pub(crate) line: usize,
    /// Where it applies: `None` everywhere.
    pub(crate) scope: Option<Scope>,
}

/// Where a scoped deviation applies: to violations in a file one of `paths`
/// matches, or in a crate `crates` names.
#[derive(Debug)]
pub(crate) struct Scope {
    pub(crate) paths: Vec<(GlobMatcher, Listed)>,
    pub(crate) crates: Vec<Listed>,
}

/// A value of a list in `standard.toml`, with its line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Listed {
    pub(crate) value: String,
    /// Its line, from 1.
    pub(crate) line: usize,
}

/// One entry of a deviation that excuses no violation, which can go.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Stale {
    /// The whole deviation, at this line.
    Whole(usize),
    /// A glob of its `paths`.
    Path(Listed),
    /// A crate of its `crates`.
    Crate(Listed),
}

impl Deviation {
    /// Whether it excuses a violation in `path`, in the crate `krate`.
    pub(crate) fn excuses(&self, path: &Path, krate: Option<&str>) -> bool {
        self.scope
            .as_ref()
            .is_none_or(|scope| scope.excuses(path, krate))
    }

    /// The entries that excuse none of `excused`, the files and crates of
    /// the violations it applied to.
    pub(crate) fn stale(&self, excused: &[(&Path, Option<&str>)]) -> Vec<Stale> {
        match &self.scope {
            None if excused.is_empty() => vec![Stale::Whole(self.line)],
            None => Vec::new(),
            Some(scope) => {
                let paths = scope
                    .paths
                    .iter()
                    .filter(|(glob, _)| !excused.iter().any(|(path, _)| glob.is_match(path)))
                    .map(|(_, listed)| Stale::Path(listed.clone()));
                let crates = scope
                    .crates
                    .iter()
                    .filter(|listed| {
                        !excused
                            .iter()
                            .any(|(_, krate)| *krate == Some(listed.value.as_str()))
                    })
                    .map(|listed| Stale::Crate(listed.clone()));
                paths.chain(crates).collect()
            }
        }
    }
}

impl Scope {
    fn excuses(&self, path: &Path, krate: Option<&str>) -> bool {
        self.paths.iter().any(|(glob, _)| glob.is_match(path))
            || self
                .crates
                .iter()
                .any(|listed| Some(listed.value.as_str()) == krate)
    }
}

/// The `[check-changed]` table.
#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct CheckChanged {
    /// Crates that build another without depending on it, which
    /// `cargo metadata` doesn't show, such as a build script compiling a
    /// crate it embeds.
    pub(crate) builds: Vec<Builds>,
}

/// A crate that builds another without depending on it.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Builds {
    /// The crate that builds `built`, so a change reaching `built` reaches
    /// it too.
    pub(crate) builder: String,
    /// The crate it builds.
    pub(crate) built: String,
}

/// `standard.toml` as written, before its spans become lines.
#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
struct File {
    /// Each rule the repository departs from, and why.
    deviations: BTreeMap<Spanned<Rule>, Written>,
    check_changed: CheckChanged,
}

/// A deviation as written: its reason alone, for everywhere, or a table
/// with its reason and scope.
enum Written {
    Everywhere(String),
    Scoped(WrittenScope),
}

/// A scoped deviation as written.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WrittenScope {
    reason: String,
    #[serde(default)]
    paths: Vec<Spanned<String>>,
    #[serde(default)]
    crates: Vec<Spanned<String>>,
}

impl<'de> Deserialize<'de> for Written {
    /// A string, or a table read as [`WrittenScope`], so its errors still
    /// name the bad key.
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = Written;

            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("a reason, or a table of `reason`, `paths` and `crates`")
            }

            fn visit_str<E: serde::de::Error>(self, reason: &str) -> Result<Written, E> {
                Ok(Written::Everywhere(reason.to_owned()))
            }

            fn visit_map<M: serde::de::MapAccess<'de>>(self, map: M) -> Result<Written, M::Error> {
                WrittenScope::deserialize(serde::de::value::MapAccessDeserializer::new(map))
                    .map(Written::Scoped)
            }
        }
        deserializer.deserialize_any(Visitor)
    }
}

impl Config {
    /// Loads `standard.toml` under `root`, or the defaults when there is
    /// none.
    pub(crate) fn load(root: &Path) -> Result<Self, ConfigError> {
        let path = root.join(PATH);
        match std::fs::read_to_string(&path) {
            Ok(text) => Self::parse(&text),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(source) => Err(ConfigError::Read { path, source }),
        }
    }

    /// Parses `standard.toml`'s text.
    pub(crate) fn parse(text: &str) -> Result<Self, ConfigError> {
        let file: File = toml::from_str(text).map_err(|error| ConfigError::Parse {
            message: error.to_string(),
        })?;
        let listed = |value: Spanned<String>| Listed {
            line: line_of(text, value.span().start),
            value: value.into_inner(),
        };
        let deviations = file
            .deviations
            .into_iter()
            .map(|(rule, written)| {
                let line = line_of(text, rule.span().start);
                let rule = rule.into_inner();
                let (reason, scope) = match written {
                    Written::Everywhere(reason) => (reason, None),
                    Written::Scoped(scope) => {
                        if scope.paths.is_empty() && scope.crates.is_empty() {
                            return Err(ConfigError::NoScope {
                                rule: rule.name(),
                                line,
                            });
                        }
                        let paths = scope
                            .paths
                            .into_iter()
                            .map(|glob| {
                                let listed = listed(glob);
                                GlobBuilder::new(&listed.value)
                                    .literal_separator(true)
                                    .build()
                                    .map(|glob| (glob.compile_matcher(), listed.clone()))
                                    .map_err(|source| ConfigError::Glob {
                                        line: listed.line,
                                        source,
                                    })
                            })
                            .collect::<Result<_, _>>()?;
                        let crates = scope.crates.into_iter().map(listed).collect();
                        (scope.reason, Some(Scope { paths, crates }))
                    }
                };
                if reason.trim().is_empty() {
                    return Err(ConfigError::NoReason {
                        rule: rule.name(),
                        line,
                    });
                }
                Ok(Deviation {
                    rule,
                    reason,
                    line,
                    scope,
                })
            })
            .collect::<Result<_, _>>()?;
        Ok(Self {
            deviations,
            check_changed: file.check_changed,
        })
    }

    /// The deviation from `rule`, if the repository departs from it.
    pub(crate) fn deviation(&self, rule: Rule) -> Option<&Deviation> {
        self.deviations
            .iter()
            .find(|deviation| deviation.rule == rule)
    }
}

/// The line, from 1, holding byte `offset` of `text`.
pub(crate) fn line_of(text: &str, offset: usize) -> usize {
    text.get(..offset)
        .map_or(0, |before| before.matches('\n').count())
        + 1
}
