use super::{Fixture, INHERITS};
use crate::rules::Rule;

#[test]
fn every_migration_says_what_it_changes() {
    let fixture = Fixture::new();
    fixture
        .add_crate("core", INHERITS)
        .write(
            "standard.toml",
            "migrations = [\"crates/*/migrations/*.sql\"]\n",
        )
        .write(
            "crates/core/migrations/0001_apps.sql",
            "-- Adds the apps table; no data exists yet.\nCREATE TABLE apps (id INTEGER);\n",
        )
        .write(
            "crates/core/migrations/0002_names.sql",
            "ALTER TABLE apps ADD COLUMN name TEXT;\n",
        )
        .write("crates/core/src/query.sql", "SELECT 1;\n");
    assert_eq!(
        fixture.violations(Rule::MigrationHeader),
        [
            "crates/core/migrations/0002_names.sql:1: the migration doesn't start with a `--` \
          comment: say what it changes, and what happens to the data already there \
          (migration-header)"
        ]
    );
}
