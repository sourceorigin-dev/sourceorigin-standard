//! The justfile's modules, as `just --dump --dump-format json` lists them,
//! to tell a recipe path such as `web test` from a recipe's arguments.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;

use super::Error;
use crate::workspace::Workspace;

/// Where the justfile lives, from the repository's root.
const PATH: &str = "justfile";

/// A justfile, or one of its modules: the modules and recipes it holds.
#[derive(Debug, Default, Deserialize)]
pub(super) struct Modules {
    /// Each module, by its name.
    #[serde(default)]
    modules: BTreeMap<String, Modules>,
    /// Each recipe, by its name, as `just` describes it.
    #[serde(default)]
    recipes: BTreeMap<String, serde_json::Value>,
}

impl Modules {
    /// The modules of the justfile under `root`, none when there is no
    /// justfile.
    pub(super) fn load(root: &Path) -> Result<Self, Error> {
        if !root.join(PATH).is_file() {
            return Ok(Self::default());
        }
        let json = Workspace::new(root).output("just", &["--dump", "--dump-format", "json"])?;
        serde_json::from_str(&json).map_err(|_| Error::Shape {
            file: PATH,
            expected: "a `just --dump --dump-format json` listing its modules and recipes",
        })
    }

    /// The recipe path the words after `just` name, such as `web test` for
    /// `just web test --fast`, or `web` for `just web`, which runs the
    /// module's default recipe: after a module comes the module or recipe it
    /// holds, and anything else is an argument. `web::test` is `web test`.
    /// A recipe named by an expression, `just ${{ … }}`, isn't one.
    pub(super) fn path(&self, words: &[&str]) -> Option<String> {
        let mut words = words.iter().flat_map(|word| word.split("::"));
        let first = words.next().filter(|word| !word.starts_with('$'))?;
        let mut path = vec![first];
        let mut module = self.modules.get(first);
        while let Some(current) = module {
            let Some(next) = words.next().filter(|word| {
                current.modules.contains_key(*word) || current.recipes.contains_key(*word)
            }) else {
                break;
            };
            path.push(next);
            module = current.modules.get(next);
        }
        Some(path.join(" "))
    }
}
