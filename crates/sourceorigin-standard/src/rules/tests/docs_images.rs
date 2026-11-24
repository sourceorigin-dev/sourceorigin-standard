use super::{Fixture, INHERITS};
use crate::rules::Rule;

#[test]
fn every_image_says_what_it_shows() {
    let fixture = Fixture::new();
    fixture.add_crate("core", INHERITS).write(
        "docs/guide.md",
        "# Guide\n\n![The dashboard, listing two apps](dashboard.png)\n![](blank.png)\n\
         <img src=\"a.png\" alt=\"The sign-in page\">\n<img src=\"b.png\">\n\nWrite `![](x.png)` with alt text.\n\n```md\n![](example.png)\n```\n",
    );
    let blank = |line: usize| {
        format!(
            "docs/guide.md:{line}: an image without alt text: say what it shows, as `![what it \
             shows](…)` or `alt=\"what it shows\"` (docs-images)"
        )
    };
    assert_eq!(fixture.violations(Rule::DocsImages), [blank(4), blank(6)]);
}

#[test]
fn synced_skills_are_the_crates() {
    let fixture = Fixture::new();
    fixture
        .add_crate("core", INHERITS)
        .write(
            ".agents/skills/docs/SKILL.md",
            "---\nname: docs\n---\n\n<!-- Synced from the sourceorigin-standard crate by \
             `cargo xtask standard skills`: change it there, not here. -->\n![](shot.png)\n",
        )
        .write(
            ".agents/skills/docs/references/guide.md",
            "![](diagram.png)\n",
        )
        .write(
            ".agents/skills/own/SKILL.md",
            "---\nname: own\n---\n![](own.png)\n",
        );
    assert_eq!(
        fixture.violations(Rule::DocsImages),
        [
            ".agents/skills/own/SKILL.md:4: an image without alt text: say what it shows, as \
          `![what it shows](…)` or `alt=\"what it shows\"` (docs-images)"
        ]
    );
}
