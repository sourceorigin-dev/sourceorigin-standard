use super::{Fixture, INHERITS};
use crate::rules::Rule;

/// A file of every kind of comment, and comment markers inside literals.
const COMMENTS: &str = r##"//! Module docs.

// A plain comment,
// continued.
fn a() {
    let text = "// in a string"; /* a block */
    // SAFETY: a marked comment.
    let raw = r#"/* in a raw string */"#; // TODO: not part of the SAFETY: comment.
    let slash = '/';
    // INVARIANT: a fact the code relies on.
    /* nested /* inner */ still in it */
}
//// Four slashes.
/// Item docs.
fn b() {}
// wall-clock: the server only says it is up by answering.
#[test]
fn c() {}
"##;

#[test]
fn only_doc_comments_and_marked_comments_pass() {
    let fixture = Fixture::new();
    fixture
        .add_crate("core", INHERITS)
        .write("crates/core/src/lib.rs", COMMENTS)
        .write(
            ".gitattributes",
            "crates/core/src/generated.rs linguist-generated\n",
        )
        .write("crates/core/src/generated.rs", "// Generated: skipped.\n");
    let plain = |line: usize| {
        format!(
            "crates/core/src/lib.rs:{line}: a plain comment: move it into the item's `///` \
             docs, or remove it; only SAFETY:, INVARIANT: and wall-clock: comments stay \
             (rust-comments)"
        )
    };
    assert_eq!(
        fixture.violations(Rule::RustComments),
        [plain(3), plain(6), plain(8), plain(11), plain(13)]
    );
}

#[test]
fn docs_span_at_most_the_configured_lines() {
    let fixture = Fixture::new();
    fixture
        .add_crate("core", INHERITS)
        .write("standard.toml", "[comments]\ndoc-lines = 3\n")
        .write(
            "crates/core/src/lib.rs",
            "//! The crate's docs.\n//! Two.\n//! Three.\n//! Four.\n\nmod long;\n\n\
             /// One.\n/// Two.\n#[derive(Debug)]\n/// Three.\n/// Four.\npub struct Long;\n\
             /** One.\nTwo. */\npub fn block() {}\n\
             /// One.\n/// ```\n/// let code = 1;\n/// let more = 2;\n/// ```\n/// Two.\n\
             pub fn example() {}\n",
        )
        .write(
            "crates/core/src/long.rs",
            "//! One.\n//! Two.\n//! Three.\n//! Four.\n",
        );
    let long = |path: &str, line: usize| {
        format!(
            "crates/core/src/{path}:{line}: docs of 4 lines, code blocks aside, over the 3 of \
             standard.toml's `[comments] doc-lines`: keep what a reader of the item needs, and \
             move the rest to the crate's docs (doc-length)"
        )
    };
    assert_eq!(
        fixture.violations(Rule::DocLength),
        [long("lib.rs", 8), long("long.rs", 1)]
    );
}
