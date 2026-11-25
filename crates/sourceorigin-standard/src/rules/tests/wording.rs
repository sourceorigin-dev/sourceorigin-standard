use super::{Fixture, INHERITS};
use crate::rules::Rule;

#[test]
fn the_docs_use_the_names_the_repository_prefers() {
    let fixture = Fixture::new();
    fixture
        .add_crate("core", INHERITS)
        .write(
            "standard.toml",
            "[wording]\n\"Source Origin\" = \"SourceOrigin\"\n\"Origin auth\" = \"Origin Auth\"\n",
        )
        .write(
            "docs/guide.md",
            "# Source Origin\n\nSign in with Origin auth, which `Source Origin` names.\n\
             Origin authority is a word of its own.\n\n```\nSource Origin\n```\n",
        );
    assert_eq!(
        fixture.violations(Rule::Wording),
        [
            "docs/guide.md:1: write `SourceOrigin`, not `Source Origin` (wording)",
            "docs/guide.md:3: write `Origin Auth`, not `Origin auth` (wording)",
        ]
    );
    fixture.fix_rule(Rule::Wording);
    let guide =
        std::fs::read_to_string(fixture.0.root().join("docs/guide.md")).expect("the page is read");
    assert!(
        guide.starts_with(
            "# SourceOrigin\n\nSign in with Origin Auth, which `Source Origin` names.\n"
        ),
        "{guide}"
    );
}

#[test]
fn only_words_in_prose_are_rewritten() {
    let fixture = Fixture::new();
    fixture
        .add_crate("core", INHERITS)
        .write("standard.toml", "[wording]\n\"repo\" = \"repository\"\n")
        .write(
            "docs/guide.md",
            "See [the repo](https://github.com/org/repo); a repo holds reports.\n\
             Clone <https://example.com/repo>, or https://example.com/repo, not `repo`.\n",
        )
        .fix_rule(Rule::Wording);
    let guide =
        std::fs::read_to_string(fixture.0.root().join("docs/guide.md")).expect("the page is read");
    assert_eq!(
        guide,
        "See [the repository](https://github.com/org/repo); a repository holds reports.\n\
         Clone <https://example.com/repo>, or https://example.com/repo, not `repo`.\n"
    );
}

#[test]
fn a_word_joined_to_another_is_left_alone() {
    let fixture = Fixture::new();
    fixture
        .add_crate("core", INHERITS)
        .write("standard.toml", "[wording]\n\"lib\" = \"crate\"\n")
        .write(
            "docs/guide.md",
            "The lib, my_lib, lib-foo and Lib.\n\
             Mail lib@example.com, visit lib.fr or <a href=\"lib.html\">lib</a>.\n",
        )
        .fix_rule(Rule::Wording);
    let guide =
        std::fs::read_to_string(fixture.0.root().join("docs/guide.md")).expect("the page is read");
    assert_eq!(
        guide,
        "The crate, my_lib, crate-foo and Lib.\n\
         Mail lib@example.com, visit lib.fr or <a href=\"lib.html\">crate</a>.\n"
    );
}

#[test]
fn doc_comments_are_read_as_prose() {
    let fixture = Fixture::new();
    fixture.add_crate("core", INHERITS).write(
        "crates/core/src/lib.rs",
        "//! Additionally, it parses.\n\n/// We utilize `utilize`.\n///\n/// ```\n\
             /// utilize();\n/// ```\npub fn f() {\n    let _ = \"/// utilize\";\n}\n\
             #[doc = \"Utilize it.\"]\npub fn g() {}\n",
    );
    assert_eq!(
        fixture.violations(Rule::Wording),
        [
            "crates/core/src/lib.rs:1: write `Also`, not `Additionally` (wording)",
            "crates/core/src/lib.rs:3: write `use`, not `utilize` (wording)",
        ]
    );
    fixture.fix_rule(Rule::Wording);
    let lib = std::fs::read_to_string(fixture.0.root().join("crates/core/src/lib.rs"))
        .expect("the file is read");
    assert!(
        lib.starts_with(
            "//! Also, it parses.\n\n/// We use `utilize`.\n///\n/// ```\n/// utilize();\n"
        ),
        "{lib}"
    );
}

#[test]
fn a_repository_keeps_a_default_by_preferring_it_to_itself() {
    let fixture = Fixture::new();
    fixture
        .add_crate("core", INHERITS)
        .write("standard.toml", "[wording]\n\"leverage\" = \"leverage\"\n")
        .write("docs/guide.md", "In order to lift it, use leverage.\n");
    assert_eq!(
        fixture.violations(Rule::Wording),
        ["docs/guide.md:1: write `To`, not `In order to` (wording)"]
    );
}
