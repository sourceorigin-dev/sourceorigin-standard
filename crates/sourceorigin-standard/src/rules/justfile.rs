//! The repository's justfile, as `just --dump --dump-format json` describes
//! it.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;

use crate::workspace::Workspace;

/// The justfile, from the repository's root.
pub(super) const PATH: &str = "justfile";

/// The justfile's recipes.
#[derive(Debug, Deserialize)]
pub(super) struct Justfile {
    /// The recipe `just` alone runs.
    pub(super) first: Option<String>,
    pub(super) recipes: BTreeMap<String, Recipe>,
}

/// A recipe.
#[derive(Debug, Deserialize)]
pub(super) struct Recipe {
    /// Each line, as fragments: text, or an interpolation.
    body: Vec<Vec<serde_json::Value>>,
    pub(super) dependencies: Vec<Dependency>,
    pub(super) parameters: Vec<Parameter>,
}

/// A recipe another runs first.
#[derive(Debug, Deserialize)]
pub(super) struct Dependency {
    pub(super) recipe: String,
}

/// A recipe's parameter.
#[derive(Debug, Deserialize)]
pub(super) struct Parameter {
    /// `singular`, `plus` or `star`.
    pub(super) kind: String,
    /// Its default, a string unless it is an expression.
    pub(super) default: Option<serde_json::Value>,
}

impl Justfile {
    /// The justfile under `root`: `None` when there is none, or when
    /// `just --dump` can't read it, which `config-syntax` reports.
    pub(super) fn load(root: &Path) -> Option<Self> {
        if !root.join(PATH).is_file() {
            return None;
        }
        let json = Workspace::new(root)
            .output("just", &["--dump", "--dump-format", "json"])
            .ok()?;
        serde_json::from_str(&json).ok()
    }

    /// The names of the recipes `name` depends on, in order.
    pub(super) fn dependencies(&self, name: &str) -> Vec<&str> {
        self.recipes.get(name).map_or_else(Vec::new, |recipe| {
            recipe
                .dependencies
                .iter()
                .map(|dependency| dependency.recipe.as_str())
                .collect()
        })
    }
}

impl Recipe {
    /// Its body's lines, each interpolation written `{{…}}`.
    pub(super) fn lines(&self) -> impl Iterator<Item = String> {
        self.body.iter().map(|fragments| {
            fragments
                .iter()
                .map(|fragment| fragment.as_str().unwrap_or("{{…}}"))
                .collect()
        })
    }
}
