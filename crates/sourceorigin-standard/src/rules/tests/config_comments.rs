use super::{Fixture, INHERITS};
use crate::rules::Rule;

/// `lines` lines of `#` comment.
fn comment(lines: usize) -> String {
    "# A comment.\n".repeat(lines)
}

#[test]
fn comments_are_capped_per_block_and_per_file() {
    let fixture = Fixture::new();
    fixture
        .add_crate("core", &format!("{}{INHERITS}", comment(7)))
        .write("standard.toml", "[comments]\nfile-lines = 12\n")
        .write(
            "justfile",
            &format!(
                "{}lint:\n    #!/usr/bin/env bash\n    cargo clippy # trailing\n",
                comment(7)
            ),
        )
        .write("just/db.just", &comment(2))
        .write(
            ".github/workflows/ci.yml",
            &format!(
                "{}name: CI\n{}on: push\n\n{}",
                comment(5),
                comment(5),
                comment(5)
            ),
        )
        .write("notes.yml", &comment(20))
        .write(
            ".github/ISSUE_TEMPLATE/bug.yml",
            &format!(
                "body:\n  - type: textarea\n    attributes:\n      value: |\n{}\n        \
                 Text.\n  - type: input\n",
                "        # A Markdown heading.\n".repeat(8)
            ),
        )
        .write(
            ".github/workflows/lint.yml",
            &format!(
                "jobs:\n  lint:\n    steps:\n      - run: >-\n{}          just lint\n",
                "          # A shell comment.\n".repeat(8)
            ),
        );
    assert_eq!(
        fixture.violations(Rule::ConfigComments),
        [
            ".github/workflows/ci.yml:1: 15 comment lines, over the 12 of standard.toml's \
             `[comments] file-lines`: shorten them, and move the rest to the docs \
             (config-comments)",
            "crates/core/Cargo.toml:4: a comment of 7 lines, over the 6 of standard.toml's \
             `[comments] block-lines`: keep the line it explains needs, and move the \
             rest to the docs (config-comments)",
            "justfile:1: a comment of 7 lines, over the 6 of standard.toml's \
             `[comments] block-lines`: keep the line it explains needs, and move the \
             rest to the docs (config-comments)",
        ]
    );
}
