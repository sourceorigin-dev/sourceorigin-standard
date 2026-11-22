//! `no-json-macro`: the crates `standard.toml` lists build JSON from typed
//! structs, never `json!`.

use std::path::Path;

use super::walk::{Inspect, Scope, Walk};
use super::{Finding, Repository};
use crate::config;

/// The `json!` invocations outside test code in one file.
struct JsonMacros<'a> {
    path: &'a Path,
    findings: Vec<Finding>,
}

impl Inspect for JsonMacros<'_> {
    fn macro_call(&mut self, call: &syn::Macro, scope: Scope) {
        let Some(last) = call.path.segments.last() else {
            return;
        };
        if scope.test || last.ident != "json" {
            return;
        }
        self.findings.push(Finding {
            path: self.path.to_owned(),
            line: last.ident.span().start().line,
            message: "`json!` builds a document no type checks: serialize a struct that \
                      describes it"
                .to_owned(),
        });
    }
}

impl Repository {
    /// Each `json!` outside test code in a crate `no-json-macro` lists, and
    /// each listed name that is no workspace crate.
    pub(super) fn no_json_macro(&self) -> Vec<Finding> {
        let listed = &self.config.no_json_macro;
        let mut findings: Vec<Finding> = listed
            .iter()
            .filter(|entry| !self.metadata.has(&entry.value))
            .map(|entry| Finding {
                path: config::PATH.into(),
                line: entry.line,
                message: format!(
                    "`{}` is in `no-json-macro`, but is no workspace crate: remove it",
                    entry.value
                ),
            })
            .collect();
        for source in &self.sources {
            let in_listed = self
                .crate_of(&source.path)
                .is_some_and(|krate| listed.iter().any(|entry| entry.value == krate));
            if !in_listed {
                continue;
            }
            let mut macros = JsonMacros {
                path: &source.path,
                findings: Vec::new(),
            };
            Walk::file(source, self.modules.is_test(&source.path), &mut macros);
            findings.extend(macros.findings);
        }
        findings
    }
}
