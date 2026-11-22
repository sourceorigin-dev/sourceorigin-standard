use super::{Fixture, INHERITS};
use crate::rules::{Rule, fix};

/// `count` test functions, indented by `indent`, each on three lines.
fn tests(count: usize, indent: &str) -> String {
    format!("{indent}#[test]\n{indent}fn runs() {{\n{indent}}}\n").repeat(count)
}

/// A `#[cfg(test)] mod tests` of `count` tests, its body ending in `last`.
fn module(count: usize, last: &str) -> String {
    format!(
        "#[cfg(test)]\nmod tests {{\n{}{last}}}\n",
        tests(count, "    ")
    )
}

/// The violation of the module at `line` of `path`, `span` of `of` lines,
/// moving to `to`, with `withheld` saying why `--fix` leaves it.
fn oversized(path: &str, line: usize, span: usize, of: usize, to: &str, withheld: &str) -> String {
    format!(
        "crates/core/src/{path}:{line}: the inline tests of `tests` span {span} of the file's {of} \
         lines, over 30%: move them to `crates/core/src/{to}`{} (inline-tests)",
        if withheld.is_empty() {
            String::new()
        } else {
            format!(" (by hand: `--fix` doesn't, as {withheld})")
        }
    )
}

#[test]
fn inline_tests_over_30_percent_of_their_file_move_to_a_file_of_their_own() {
    let literal =
        "    const TEXT: &str = \"a\n  b\";\n    const DATA: &str = include_str!(\"data.txt\");\n";
    let big = format!(
        "pub fn one() -> u8 {{\n    1\n}}\n\n{}",
        module(16, literal)
    );
    let fixture = Fixture::new();
    fixture
        .add_crate("core", INHERITS)
        .write(
            "crates/core/src/lib.rs",
            "mod big;\nmod includes;\nmod nested;\nmod small;\nmod shared;\nmod spread;\n",
        )
        .write("crates/core/src/big.rs", &big)
        .write("crates/core/src/nested/mod.rs", &module(16, ""))
        .write(
            "crates/core/src/small.rs",
            &format!("pub fn f() {{}}\n{}", module(10, "")),
        )
        .write(
            "crates/core/src/includes.rs",
            &format!(
                "const DATA: &str = include_str!(\"data.txt\");\n{}",
                module(16, "")
            ),
        )
        .write("crates/core/src/shared/mod.rs", &module(16, ""))
        .write("crates/core/src/shared/tests/fixture.json", "{}\n")
        .write(
            "crates/core/src/spread.rs",
            &format!(
                "#[cfg(test)]\nmod tests {{ #[test]\n{}}}\n",
                tests(16, "    ")
            ),
        )
        .write("crates/core/tests/it.rs", &module(16, ""));
    assert_eq!(
        fixture.violations(Rule::InlineTests),
        [
            oversized("big.rs", 5, 54, 58, "big/tests.rs", ""),
            oversized(
                "includes.rs",
                2,
                51,
                52,
                "includes/tests.rs",
                "moving it would change what a relative path in it names"
            ),
            oversized("nested/mod.rs", 1, 51, 51, "nested/tests.rs", ""),
            oversized(
                "shared/mod.rs",
                1,
                51,
                51,
                "shared/tests.rs",
                "`crates/core/src/shared/tests/` exists"
            ),
            oversized(
                "spread.rs",
                1,
                51,
                51,
                "spread/tests.rs",
                "its braces share their lines with code"
            ),
        ]
    );
    assert_eq!(
        fixture.violations(Rule::ChildlessModRs),
        Vec::<String>::new()
    );

    fix(fixture.0.root()).expect("the fixes are made");
    let read = |path: &str| {
        std::fs::read_to_string(fixture.0.root().join("crates/core/src").join(path))
            .expect("the file is read")
    };
    assert_eq!(
        read("big/mod.rs"),
        "pub fn one() -> u8 {\n    1\n}\n\n#[cfg(test)]\nmod tests;\n"
    );
    assert_eq!(
        read("big/tests.rs"),
        format!(
            "{}const TEXT: &str = \"a\n  b\";\nconst DATA: &str = include_str!(\"../data.txt\");\n",
            tests(16, "")
        )
    );
    assert_eq!(read("nested/mod.rs"), "#[cfg(test)]\nmod tests;\n");
    assert_eq!(read("nested/tests.rs"), tests(16, ""));
    assert_eq!(fixture.violations(Rule::InlineTests).len(), 3);
    assert_eq!(
        fixture.violations(Rule::ChildlessModRs),
        Vec::<String>::new()
    );
}

#[test]
fn every_include_a_moved_module_holds_reads_one_directory_up() {
    let fixture = Fixture::new();
    let includes = "    const RAW: &str = include_str!(r#\"data.txt\"#);\n    \
                    const SPLIT: &[u8] = include_bytes!(\n        \"data.txt\"\n    );\n";
    fixture
        .add_crate("core", INHERITS)
        .write("crates/core/src/lib.rs", "mod foo;\n")
        .write("crates/core/src/data.txt", "x")
        .write(
            "crates/core/src/foo.rs",
            &format!("pub fn f() {{}}\n{}", module(20, includes)),
        );
    fix(fixture.0.root()).expect("the fixes are made");
    let moved = std::fs::read_to_string(fixture.0.root().join("crates/core/src/foo/tests.rs"))
        .expect("the tests moved");
    assert!(
        moved.ends_with(
            "const RAW: &str = include_str!(r#\"../data.txt\"#);\nconst SPLIT: &[u8] = \
             include_bytes!(\n    \"../data.txt\"\n);\n"
        ),
        "{moved}"
    );
}

#[test]
fn a_file_a_path_attribute_names_keeps_its_tests_by_hand() {
    let fixture = Fixture::new();
    fixture
        .add_crate("core", INHERITS)
        .write(
            "crates/core/src/lib.rs",
            "#[path = \"platform_linux.rs\"]\nmod platform;\n",
        )
        .write(
            "crates/core/src/platform_linux.rs",
            &format!("pub fn f() {{}}\n{}", module(20, "")),
        );
    assert_eq!(
        fixture.violations(Rule::InlineTests),
        [oversized(
            "platform_linux.rs",
            2,
            63,
            64,
            "platform_linux/tests.rs",
            "a `#[path]` attribute names its file, which would move"
        )]
    );
}
