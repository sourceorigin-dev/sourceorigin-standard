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
