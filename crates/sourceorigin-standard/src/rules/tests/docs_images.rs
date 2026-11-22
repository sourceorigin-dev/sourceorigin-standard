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
