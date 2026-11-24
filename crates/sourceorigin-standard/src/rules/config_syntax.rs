//! `config-syntax`: the configuration files the rules read parse, so a file
//! that doesn't is a violation of its own while the other rules still run.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use yaml_rust2::YamlLoader;

use super::source::Unparsed;
use super::{Error, Finding, Repository};
use crate::config::line_of;
use crate::workspace::Workspace;

/// The TOML files the standard knows, beside the manifests and
/// `budget.toml`, which their rules load.
const TOML: [&str; 8] = [
    "deny.toml",
    "rustfmt.toml",
    ".rustfmt.toml",
    "clippy.toml",
    ".clippy.toml",
    "rust-toolchain.toml",
    ".config/nextest.toml",
    ".cargo/config.toml",
];

/// The workflows and other YAML files of `.github`, as git pathspecs.
const YAML: [&str; 2] = [":(glob).github/**/*.yml", ":(glob).github/**/*.yaml"];

/// T3 Code's scripts.
const JSON: &str = "t3.json";

/// The justfile `just --dump` reads, with the modules it imports.
const JUSTFILE: &str = "justfile";

/// The configuration files that don't parse, by path: one violation each.
#[derive(Debug, Default)]
pub(super) struct Unreadable(BTreeMap<PathBuf, Unparsed>);

impl Unreadable {
    /// Records that `path` doesn't parse, at `line`, for `message`, unless
    /// it is already recorded.
    pub(super) fn record(&mut self, path: PathBuf, line: usize, message: String) {
        self.0.entry(path.clone()).or_insert(Unparsed {
            path,
            line,
            message,
        });
    }

    /// Whether `path`, from the root, doesn't parse.
    pub(super) fn contains(&self, path: &str) -> bool {
        self.0.contains_key(Path::new(path))
    }

    /// Records each other configuration file under `root` that doesn't
    /// parse as its format: the TOML files, `.github`'s YAML, `t3.json`,
    /// and the justfile, as `just --dump` reads it.
    pub(super) fn check_others(&mut self, root: &Path) -> Result<(), Error> {
        for path in TOML {
            if let Some(text) = Self::read(root, path)?
                && let Err(error) = toml::from_str::<toml::Table>(&text)
            {
                let line = error.span().map_or(1, |span| line_of(&text, span.start));
                self.record(path.into(), line, error.message().to_owned());
            }
        }
        for path in Workspace::new(root).files(&YAML)? {
            if let Some(text) = Self::read(root, &path)?
                && let Err(error) = YamlLoader::load_from_str(&text)
            {
                self.record(path.into(), error.marker().line(), error.to_string());
            }
        }
        if let Some(text) = Self::read(root, JSON)?
            && let Err(error) = serde_json::from_str::<serde_json::Value>(&text)
        {
            self.record(JSON.into(), error.line(), error.to_string());
        }
        if root.join(JUSTFILE).exists() {
            self.check_justfile(root);
        }
        Ok(())
    }

    /// Records the justfile, or the module of it that fails, when
    /// `just --dump` can't read it, and the justfile when `just` itself is
    /// missing.
    fn check_justfile(&mut self, root: &Path) {
        let output = Workspace::new(root)
            .command("just")
            .args(["--dump", "--dump-format", "json"])
            .output();
        let output = match output {
            Ok(output) if output.status.success() => return,
            Ok(output) => output,
            Err(error) => {
                self.record(
                    JUSTFILE.into(),
                    1,
                    format!(
                        "`just` couldn't run ({error}), so the justfile can't be read: install it"
                    ),
                );
                return;
            }
        };
        let stderr = String::from_utf8_lossy(&output.stderr);
        let message = stderr
            .lines()
            .next()
            .unwrap_or("just --dump failed")
            .to_owned();
        let (path, line) = stderr
            .lines()
            .find_map(|line| {
                let place = line.split_once("▶ ")?.1.trim();
                let mut parts = place.rsplitn(3, ':');
                let (_, line, path) = (parts.next()?, parts.next()?, parts.next()?);
                Some((path.to_owned(), line.parse().ok()?))
            })
            .unwrap_or_else(|| (JUSTFILE.to_owned(), 1));
        self.record(path.into(), line, message);
    }

    /// The text of `path`, from `root`, or `None` when there is none.
    fn read(root: &Path, path: &str) -> Result<Option<String>, Error> {
        match std::fs::read_to_string(root.join(path)) {
            Ok(text) => Ok(Some(text)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(source) => Err(Error::Read {
                path: path.into(),
                source,
            }),
        }
    }
}

impl Unparsed {
    /// The file a failed `cargo metadata` couldn't read, without trusting
    /// one cargo version's wording: a file its message names (at a
    /// `--> path:line:column`, or in backticks) that doesn't parse, else any
    /// manifest or Cargo configuration git tracks that doesn't, else the
    /// line cargo points at, else the root manifest.
    pub(super) fn cargo(root: &Path, stderr: &str) -> Result<Self, Error> {
        let message = stderr
            .lines()
            .find_map(|line| line.strip_prefix("error: "))
            .unwrap_or("`cargo metadata` failed")
            .to_owned();
        let arrow = stderr.lines().find_map(|line| {
            let place = line.trim_start().strip_prefix("--> ")?;
            let mut parts = place.rsplitn(3, ':');
            let (_, line, path) = (parts.next()?, parts.next()?, parts.next()?);
            Some((PathBuf::from(path), line.parse::<usize>().ok()?))
        });
        let quoted = stderr.split('`').skip(1).step_by(2).filter_map(|quoted| {
            let path = Path::new(quoted);
            let path = path.strip_prefix(root).unwrap_or(path);
            let manifest = path.join("Cargo.toml");
            if root.join(&manifest).is_file() {
                Some(manifest)
            } else {
                root.join(path).is_file().then(|| path.to_owned())
            }
        });
        let named: Vec<PathBuf> = arrow
            .iter()
            .map(|(path, _)| path.clone())
            .chain(quoted)
            .collect();
        let tracked = Workspace::new(root).files(&[
            ":(glob)**/Cargo.toml",
            ".cargo/config.toml",
            ".cargo/config",
        ])?;
        for path in named
            .iter()
            .cloned()
            .chain(tracked.into_iter().map(PathBuf::from))
        {
            if let Some(unparsed) = Self::toml(root, &path)? {
                return Ok(unparsed);
            }
        }
        let (path, line) = arrow.unwrap_or_else(|| (PathBuf::from("Cargo.toml"), 1));
        Ok(Self {
            path,
            line,
            message,
        })
    }

    /// Where `path`, from `root`, fails to parse as TOML, if it does.
    fn toml(root: &Path, path: &Path) -> Result<Option<Self>, Error> {
        let Some(text) = Unreadable::read(root, &path.to_string_lossy())? else {
            return Ok(None);
        };
        Ok(text
            .parse::<toml_edit::DocumentMut>()
            .err()
            .map(|error| Self {
                path: path.to_owned(),
                line: error.span().map_or(1, |span| line_of(&text, span.start)),
                message: error.message().trim().to_owned(),
            }))
    }

    /// The `config-syntax` finding it is.
    pub(super) fn finding(&self) -> Finding {
        Finding {
            path: self.path.clone(),
            line: self.line,
            message: format!("{}: fix it so the rules can read it", self.message),
        }
    }
}

impl Repository {
    /// Each configuration file that doesn't parse.
    pub(super) fn config_syntax(&self) -> Vec<Finding> {
        self.unreadable.0.values().map(Unparsed::finding).collect()
    }
}
