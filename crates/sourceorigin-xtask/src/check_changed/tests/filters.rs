use std::collections::BTreeSet;

use super::super::Error;
use super::super::filters::{Filters, Quantifier};
use super::{FILTERS, Filter, paths};

/// The filters with `docs` holding `patterns` instead.
fn docs(patterns: &str, quantifier: Quantifier) -> Filters<Filter> {
    let yaml = FILTERS.replace("[docs/**]", patterns);
    Filters::parse(&yaml, quantifier).expect("the filters parse")
}

#[test]
fn globs_match_as_picomatch_does() {
    // Each checked against picomatch 2.3.1, which paths-filter 4.0.3
    // bundles, with `{ dot: true }`.
    let cases = [
        ("crates/**", "crates/core/src/lib.rs", true),
        ("crates/**", "crates/core/.env", true),
        ("crates/**", "xcrates/lib.rs", false),
        (".github/workflows/**", ".github/workflows/ci.yml", true),
        ("**/Cargo.toml", "Cargo.toml", true),
        ("**/Cargo.toml", "apps/cli/Cargo.toml", true),
        ("**/Cargo.toml", "Cargo.toml.orig", false),
        ("docs/**/*.json", "docs/schema.json", true),
        ("docs/**/*.json", "docs/user/reference/schema.json", true),
        ("*.md", "README.md", true),
        ("*.md", "docs/README.md", false),
        ("apps/*/src/*.rs", "apps/.hidden/src/lib.rs", true),
        ("apps/*/src/*.rs", "apps/cli/src/cli/mod.rs", false),
        (
            "{Cargo.toml,**/Cargo.toml,Cargo.lock}",
            "crates/core/Cargo.toml",
            true,
        ),
        ("{Cargo.toml,**/Cargo.toml,Cargo.lock}", "Cargo.lock", true),
        ("docs/{cli,variables/*}.md", "docs/variables/port.md", true),
        (
            "docs/{cli,variables/*}.md",
            "docs/variables/a/port.md",
            false,
        ),
        ("!docs/**", "docs/index.md", false),
        ("!docs/**", "src/main.rs", true),
    ];
    for (pattern, path, matches) in cases {
        let filters = docs(&format!("['{pattern}']"), Quantifier::Some);
        assert_eq!(
            filters.matches(Filter::Docs, path),
            matches,
            "{pattern} against {path}"
        );
    }
}

/// A path, and whether it matches under `some`, `every` and
/// `some-with-excludes`.
type Expected = (&'static str, bool, bool, bool);

#[test]
fn quantifiers_combine_negated_patterns_as_paths_filter_does() {
    // The examples of paths-filter 4.0.3's README, and DirkEngine's and
    // sourceorigin's filters, each checked against its `isMatch` over
    // picomatch 2.3.1: (path, some, every, some-with-excludes).
    let cases: [(&str, &[Expected]); 5] = [
        (
            "['pkg/a/b/c/**', '!**/*.jpeg', '!**/*.md']",
            &[
                ("pkg/a/b/c/main.ts", true, true, true),
                ("pkg/a/b/c/README.md", true, false, false),
                ("pkg/a/b/c/logo.jpeg", true, false, false),
                ("other/main.ts", true, false, false),
            ],
        ),
        (
            "['mobile/**', '!mobile/**/*.md', '!mobile/.config/**', \
             '.github/workflows/test_mobile.yml']",
            &[
                ("mobile/src/app.kt", true, false, true),
                ("mobile/README.md", true, false, false),
                ("mobile/.config/app.json", true, false, false),
                (".github/workflows/test_mobile.yml", true, false, true),
                ("web/app.ts", true, false, false),
            ],
        ),
        (
            "['**', '!docs/**', '!AGENTS.md', '!LICENSE', '!t3.json', '!.gitignore']",
            &[
                ("src/main.rs", true, true, true),
                ("docs/index.md", true, false, false),
                ("AGENTS.md", true, false, false),
                ("crates/core/AGENTS.md", true, true, true),
                (".github/workflows/ci.yml", true, true, true),
                (".gitignore", true, false, false),
            ],
        ),
        (
            "['**', '!**/*.{rs,sql,toml,md}', '!Cargo.lock']",
            &[
                ("packages/auth/src/index.ts", true, true, true),
                ("crates/core/src/lib.rs", true, false, false),
                ("Cargo.lock", true, false, false),
                (".prettierrc", true, true, true),
            ],
        ),
        // Only negated patterns: nothing to include them.
        (
            "['!docs/**']",
            &[
                ("src/main.rs", true, true, false),
                ("docs/index.md", false, false, false),
            ],
        ),
    ];
    for (patterns, paths) in cases {
        let [some, every, some_with_excludes] = [
            Quantifier::Some,
            Quantifier::Every,
            Quantifier::SomeWithExcludes,
        ]
        .map(|quantifier| docs(patterns, quantifier));
        for &(path, by_some, by_every, by_some_with_excludes) in paths {
            assert_eq!(
                [
                    some.matches(Filter::Docs, path),
                    every.matches(Filter::Docs, path),
                    some_with_excludes.matches(Filter::Docs, path),
                ],
                [by_some, by_every, by_some_with_excludes],
                "{patterns} against {path}"
            );
        }
    }
}

#[test]
fn aliases_include_the_filters_they_name() {
    let filters = Filters::parse(FILTERS, Quantifier::Some).expect("the filters parse");
    assert_eq!(
        filters.matching(&paths(&["Cargo.lock"])),
        BTreeSet::from([Filter::Workspace, Filter::Crates, Filter::Rust])
    );
}

#[test]
fn filters_must_name_exactly_the_enum() {
    let unknown = format!("{FILTERS}frontend: [web/**]\n");
    assert!(matches!(
        Filters::<Filter>::parse(&unknown, Quantifier::Some),
        Err(Error::UnknownFilter { name }) if name == "frontend"
    ));
    let missing = FILTERS.replace("docs: [docs/**]\n", "");
    assert!(matches!(
        Filters::<Filter>::parse(&missing, Quantifier::Some),
        Err(Error::MissingFilter { name: "docs" })
    ));
}

#[test]
fn patterns_paths_filter_matches_otherwise_are_rejected() {
    for patterns in ["['!(docs)/**']", "{added: 'docs/**'}"] {
        let yaml = FILTERS.replace("[docs/**]", patterns);
        assert!(
            matches!(
                Filters::<Filter>::parse(&yaml, Quantifier::Some),
                Err(Error::Unsupported { filter, .. }) if filter == "docs"
            ),
            "{patterns}"
        );
    }
}
