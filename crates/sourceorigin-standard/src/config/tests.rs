use super::{Config, ConfigError};
use crate::testing::Repository;

/// The parser's message for `text`, which must fail to parse.
fn message(text: &str) -> String {
    match Config::parse(text) {
        Err(ConfigError::Parse { message }) => message,
        other => panic!("{text} parsed: {other:?}"),
    }
}

#[test]
fn a_missing_file_is_the_defaults() {
    let repository = Repository::new();
    let config = Config::load(repository.root()).expect("no file loads");
    assert_eq!(config.deviations.len(), 0);
    assert_eq!(config.check_changed.builds.len(), 0);
}

#[test]
fn check_changed_reads_the_crates_that_build_others() {
    let config = Config::parse(
        "[check-changed]\nbuilds = [{ builder = \"daemon\", built = \"dashboard\" }]\n",
    )
    .expect("the builds parse");
    let builds = &config.check_changed.builds;
    assert_eq!(
        builds
            .iter()
            .map(|builds| (builds.builder.as_str(), builds.built.as_str()))
            .collect::<Vec<_>>(),
        [("daemon", "dashboard")]
    );
}

#[test]
fn errors_name_the_bad_key_and_its_line() {
    let cases = [
        ("\n[deviation]\n", "unknown field `deviation`", "line 2"),
        (
            "[deviations]\nno-such-rule = \"why\"\n",
            "no-such-rule",
            "line 2",
        ),
        (
            "[check-changed]\nbuilds = [{ builder = \"daemon\" }]\n",
            "missing field `built`",
            "line 2",
        ),
        (
            "[deviations]\nworkspace-lints = { reason = \"r\", path = [\"crates/**\"] }\n",
            "unknown field `path`",
            "line 2",
        ),
        (
            "[check-changed]\nbuild = []\n",
            "unknown field `build`",
            "line 2",
        ),
    ];
    for (text, names, line) in cases {
        let message = message(text);
        assert!(
            message.contains(names) && message.contains(line),
            "{text}: {message}"
        );
    }
}
