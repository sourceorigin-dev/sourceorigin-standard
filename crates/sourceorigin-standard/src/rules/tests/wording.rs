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
