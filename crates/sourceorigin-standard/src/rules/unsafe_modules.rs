//! `unsafe-modules`: unsafe code stays in the modules `standard.toml` lists.

use std::collections::BTreeSet;

use syn::visit::{self, Visit};

use super::source::LintAttribute;
use super::{Finding, Repository};
use crate::config;

/// The `#[expect(unsafe_code)]` attributes of one file, each with the inline
/// modules around it.
#[derive(Default)]
struct Expects {
    /// The inline modules the walk is in, and the module an attribute on a
    /// `mod` item applies to.
    inline: Vec<String>,
    /// The line of each attribute, and its inline modules.
    found: Vec<(usize, Vec<String>)>,
}

impl<'ast> Visit<'ast> for Expects {
    fn visit_item_mod(&mut self, item: &'ast syn::ItemMod) {
        self.inline.push(item.ident.to_string());
        visit::visit_item_mod(self, item);
        self.inline.pop();
    }

    fn visit_attribute(&mut self, attribute: &'ast syn::Attribute) {
        if let Some(lint) = LintAttribute::parse(attribute)
            .into_iter()
            .find(|lint| lint.expects("unsafe_code"))
        {
            self.found.push((lint.line, self.inline.clone()));
        }
    }
}

impl Repository {
    /// Each `#[expect(unsafe_code)]` outside the modules `unsafe-modules`
    /// lists, and each listed module holding none.
    pub(super) fn unsafe_modules(&self) -> Vec<Finding> {
        let listed: Vec<(Vec<&str>, &config::Listed)> = self
            .config
            .unsafe_modules
            .iter()
            .map(|entry| (entry.value.split("::").collect(), entry))
            .collect();
        let mut used: BTreeSet<usize> = BTreeSet::new();
        let mut findings = Vec::new();
        for source in &self.sources {
            let mut expects = Expects::default();
            expects.visit_file(&source.syntax);
            let files = self.modules.of_file(&source.path);
            for (line, inline) in expects.found {
                let modules: Vec<Vec<&str>> = files
                    .iter()
                    .map(|module| module.iter().chain(&inline).map(String::as_str).collect())
                    .collect();
                let inside: Vec<Option<usize>> = modules
                    .iter()
                    .map(|module| {
                        listed
                            .iter()
                            .position(|(prefix, _)| module.starts_with(prefix))
                    })
                    .collect();
                used.extend(inside.iter().flatten().copied());
                if !modules.is_empty() && inside.iter().all(Option::is_some) {
                    continue;
                }
                let outside = modules
                    .iter()
                    .zip(&inside)
                    .find(|(_, listed)| listed.is_none())
                    .map(|(module, _)| module);
                let message = match outside {
                    Some(module) => format!(
                        "`{}` expects `unsafe_code`, but no module `unsafe-modules` in {} lists \
                         holds it: list the smallest module that needs it",
                        module.join("::"),
                        config::PATH
                    ),
                    None => format!(
                        "`#[expect(unsafe_code)]` in a file no target's modules reach: move it \
                         into a module `unsafe-modules` in {} lists",
                        config::PATH
                    ),
                };
                findings.push(Finding {
                    path: source.path.clone(),
                    line,
                    message,
                });
            }
        }
        findings.extend(
            listed
                .iter()
                .enumerate()
                .filter(|(index, _)| !used.contains(index))
                .map(|(_, (_, entry))| Finding {
                    path: config::PATH.into(),
                    line: entry.line,
                    message: format!(
                        "`{}` is in `unsafe-modules`, but holds no `#[expect(unsafe_code, \
                         reason = \"…\")]`: remove it",
                        entry.value
                    ),
                }),
        );
        findings
    }
}
