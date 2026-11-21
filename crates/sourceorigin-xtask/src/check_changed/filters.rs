//! The path filters in `.github/filters.yml`, matched as CI's `changes` job
//! matches them with dorny/paths-filter.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use globset::{GlobBuilder, GlobMatcher};
use yaml_rust2::{Yaml, YamlLoader};

use super::{Error, Filter};

/// Where the filters live, from the repository's root.
pub(super) const PATH: &str = ".github/filters.yml";

/// How paths-filter combines a filter's patterns, as its
/// `predicate-quantifier` input says.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Quantifier {
    /// A path matches when it matches any pattern, a negated one matching
    /// every path its glob doesn't. paths-filter's default.
    #[default]
    Some,
    /// A path matches when it matches every pattern, so negated patterns
    /// exclude paths.
    Every,
    /// A path matches when it matches a pattern that isn't negated, and no
    /// negated pattern's glob.
    SomeWithExcludes,
}

impl Quantifier {
    /// The quantifier `predicate-quantifier: <input>` names.
    pub(super) fn parse(input: &str) -> Result<Self, Error> {
        match input {
            "some" => Ok(Self::Some),
            "every" => Ok(Self::Every),
            "some-with-excludes" => Ok(Self::SomeWithExcludes),
            found => Err(Error::UnknownQuantifier {
                found: found.to_owned(),
            }),
        }
    }

    /// Whether `path` matches a filter of `patterns`.
    fn matches(self, patterns: &[Pattern], path: &str) -> bool {
        match self {
            Self::Some => patterns.iter().any(|pattern| pattern.matches(path)),
            Self::Every => patterns.iter().all(|pattern| pattern.matches(path)),
            Self::SomeWithExcludes => {
                let (excludes, includes): (Vec<_>, Vec<_>) =
                    patterns.iter().partition(|pattern| pattern.negated);
                includes.iter().any(|pattern| pattern.glob.is_match(path))
                    && !excludes.iter().any(|pattern| pattern.glob.is_match(path))
            }
        }
    }
}

/// One pattern of a filter, as picomatch compiles it with `dot: true`.
#[derive(Debug)]
struct Pattern {
    /// The glob, without the `!` that negates it.
    glob: GlobMatcher,
    /// Whether a leading `!` negates it, so it matches the paths its glob
    /// doesn't.
    negated: bool,
}

impl Pattern {
    /// Compiles `pattern` as picomatch does with `dot: true`: `*` matches
    /// within a path segment and `**` across them, both match dotfiles, and
    /// a leading `!` negates the rest. picomatch also matches a directory
    /// itself against `dir/**`, which is moot here: git lists only files.
    fn parse(filter: &str, pattern: &str) -> Result<Self, Error> {
        let (glob, negated) = match pattern.strip_prefix('!') {
            Some(glob) => (glob, true),
            None => (pattern, false),
        };
        // picomatch reads `!(...)` as an extglob, which globset reads as a
        // literal `!` and so would silently match nothing.
        if pattern.starts_with("!(") {
            return Err(Error::Unsupported {
                filter: filter.to_owned(),
                item: format!("extglob `{pattern}`"),
            });
        }
        let glob = GlobBuilder::new(glob)
            .literal_separator(true)
            .build()
            .map_err(|source| Error::Glob {
                filter: filter.to_owned(),
                source,
            })?
            .compile_matcher();
        Ok(Self { glob, negated })
    }

    /// Whether `path` matches it, as picomatch's matcher answers.
    fn matches(&self, path: &str) -> bool {
        self.glob.is_match(path) != self.negated
    }
}

/// The filters of `.github/filters.yml`, each compiled to its patterns.
#[derive(Debug)]
pub(super) struct Filters<F> {
    quantifier: Quantifier,
    patterns: BTreeMap<F, Vec<Pattern>>,
}

impl<F: Filter> Filters<F> {
    /// Loads the filters under `root`, to match with `quantifier`.
    pub(super) fn load(root: &Path, quantifier: Quantifier) -> Result<Self, Error> {
        let path = root.join(PATH);
        let yaml = std::fs::read_to_string(&path).map_err(|source| Error::Read { path, source })?;
        Self::parse(&yaml, quantifier)
    }

    /// Parses filters as paths-filter does: a map from each filter's name to
    /// a pattern or a list of them, where a list may hold another, as a YAML
    /// alias to another filter does.
    pub(super) fn parse(yaml: &str, quantifier: Quantifier) -> Result<Self, Error> {
        let documents =
            YamlLoader::load_from_str(yaml).map_err(|source| Error::Yaml { file: PATH, source })?;
        let Some(Yaml::Hash(entries)) = documents.first() else {
            return Err(Error::Shape {
                file: PATH,
                expected: "a map of filters",
            });
        };
        let mut patterns = BTreeMap::new();
        for (name, item) in entries {
            let name = name.as_str().ok_or(Error::Shape {
                file: PATH,
                expected: "a filter's name to be a string",
            })?;
            let filter = F::ALL
                .iter()
                .copied()
                .find(|filter| filter.name() == name)
                .ok_or_else(|| Error::UnknownFilter {
                    name: name.to_owned(),
                })?;
            let mut list = Vec::new();
            Self::add(name, item, &mut list)?;
            patterns.insert(filter, list);
        }
        if let Some(missing) = F::ALL.iter().find(|filter| !patterns.contains_key(filter)) {
            return Err(Error::MissingFilter {
                name: missing.name(),
            });
        }
        Ok(Self {
            quantifier,
            patterns,
        })
    }

    /// Adds the patterns `item` of `filter` lists, flattening nested lists.
    fn add(filter: &str, item: &Yaml, patterns: &mut Vec<Pattern>) -> Result<(), Error> {
        match item {
            Yaml::String(pattern) => patterns.push(Pattern::parse(filter, pattern)?),
            Yaml::Array(items) => {
                for item in items {
                    Self::add(filter, item, patterns)?;
                }
            }
            // paths-filter also takes a map from change types to patterns,
            // such as `added: '**'`, which no repository needs yet.
            other => {
                return Err(Error::Unsupported {
                    filter: filter.to_owned(),
                    item: format!("{other:?}"),
                });
            }
        }
        Ok(())
    }

    /// Whether `path`, from the repository's root, matches `filter`.
    pub(super) fn matches(&self, filter: F, path: &str) -> bool {
        self.patterns
            .get(&filter)
            .is_some_and(|patterns| self.quantifier.matches(patterns, path))
    }

    /// The filters any of `paths` matches.
    pub(super) fn matching(&self, paths: &BTreeSet<String>) -> BTreeSet<F> {
        self.patterns
            .keys()
            .copied()
            .filter(|&filter| paths.iter().any(|path| self.matches(filter, path)))
            .collect()
    }
}
