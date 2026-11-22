//! `migration-header`: each migration says what it changes, and what
//! happens to the data already there.

use super::{Finding, Repository};

impl Repository {
    /// Each migration whose first line isn't a `--` comment.
    pub(super) fn migration_header(&self) -> Vec<Finding> {
        self.migrations
            .iter()
            .filter(|migration| !migration.text.trim_start().starts_with("--"))
            .map(|migration| Finding {
                path: migration.path.clone(),
                line: 1,
                message: "the migration doesn't start with a `--` comment: say what it changes, \
                          and what happens to the data already there"
                    .to_owned(),
            })
            .collect()
    }
}
