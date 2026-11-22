//! `generated-lints`: generated files carry no lint attributes.

use super::{Finding, Repository};

impl Repository {
    /// Each lint attribute in a file `.gitattributes` marks
    /// `linguist-generated`.
    pub(super) fn generated_lints(&self) -> Vec<Finding> {
        self.sources
            .iter()
            .filter(|source| source.generated)
            .flat_map(|source| {
                source.lints.iter().map(|lint| Finding {
                    path: source.path.clone(),
                    line: lint.line,
                    message: "a generated file carries no lint attribute: put it on the \
                              file's `mod` declaration"
                        .to_owned(),
                })
            })
            .collect()
    }
}
