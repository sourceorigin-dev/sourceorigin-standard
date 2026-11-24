use std::collections::BTreeSet;
use std::path::Path;

use super::{Fixture, INHERITS};
use crate::rules::skills::{DIR, MARKER, SKILLS};
use crate::rules::{Rule, fix, sync_skills};
use crate::workspace::Workspace;

#[test]
fn every_file_of_the_skills_directory_is_embedded_and_named_for_its_skill() {
    let dir = "crates/sourceorigin-standard/skills";
    let root = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."));
    let on_disk: BTreeSet<_> = Workspace::new(root)
        .files(&[dir])
        .expect("git lists the skills")
        .into_iter()
        .map(|path| {
            Path::new(&path)
                .strip_prefix(dir)
                .expect("under skills/")
                .to_owned()
        })
        .collect();
    let embedded: BTreeSet<_> = SKILLS
        .iter()
        .flat_map(|skill| {
            skill
                .files
                .iter()
                .map(|(path, _)| Path::new(skill.name).join(path))
        })
        .collect();
    assert_eq!(embedded, on_disk, "list each file of skills/ in `skills!`");
    for skill in SKILLS {
        let (_, text) = skill
            .files
            .iter()
            .find(|(path, _)| *path == "SKILL.md")
            .expect("a SKILL.md");
        assert!(
            text.starts_with(&format!("---\nname: {}\n", skill.name)),
            "{}'s frontmatter names it",
            skill.name
        );
    }
}

#[test]
fn sync_writes_every_skill_with_its_marker_after_the_frontmatter() {
    let fixture = Fixture::new();
    fixture.add_crate("core", INHERITS);
    std::fs::remove_dir_all(fixture.0.root().join(DIR)).expect("the skills are removed");
    let missing = fixture.violations(Rule::SkillsSynced);
    assert_eq!(missing.len(), 7, "{missing:#?}");
    assert_eq!(
        missing[0],
        ".agents/skills/cleanup/SKILL.md:1: the `cleanup` skill's file is missing: run `cargo \
         xtask standard skills`, and change the skill in sourceorigin-standard instead \
         (skills-synced)"
    );

    let written = sync_skills(fixture.0.root()).expect("the skills are synced");
    assert_eq!(written.len(), 7);
    let standard = std::fs::read_to_string(fixture.0.root().join(DIR).join("standard/SKILL.md"))
        .expect("the skill is read");
    let (frontmatter, body) = standard
        .split_once(&format!("\n\n{MARKER}\n\n"))
        .expect("a marker");
    assert!(frontmatter.starts_with("---\nname: standard\n") && frontmatter.ends_with("\n---"));
    assert!(body.starts_with("# Work with the standard\n"));
    assert_eq!(fixture.violations(Rule::SkillsSynced), Vec::<String>::new());
}

#[test]
fn an_edited_copy_fails_at_its_first_changed_line_and_fix_restores_it_keeping_untracked_files() {
    let fixture = Fixture::new();
    fixture.add_crate("core", INHERITS);
    let docs = fixture.0.root().join(DIR).join("docs/SKILL.md");
    let edited = std::fs::read_to_string(&docs)
        .expect("the skill is read")
        .replacen("# Write the documentation", "# Write docs", 1);
    fixture
        .write(".agents/skills/docs/SKILL.md", &edited)
        .write(".agents/skills/docs/notes.md", "mine\n")
        .write(".gitignore", "*.swp\n")
        .write(".agents/skills/docs/.SKILL.md.swp", "swap\n")
        .write(
            ".agents/skills/old/SKILL.md",
            &format!("---\nname: old\n---\n\n{MARKER}\n"),
        )
        .write(".agents/skills/old/scripts/run.py", "print()\n")
        .write(
            ".agents/skills/repo-dev/SKILL.md",
            "---\nname: repo-dev\n---\n",
        );
    let added = std::process::Command::new("git")
        .args(["add", ".agents/skills/old"])
        .current_dir(fixture.0.root())
        .status()
        .expect("git runs");
    assert!(added.success(), "git tracks the old skill");
    let sync = "run `cargo xtask standard skills`, and change the skill in \
                sourceorigin-standard instead (skills-synced)";
    assert_eq!(
        fixture.violations(Rule::SkillsSynced),
        [
            format!(
                ".agents/skills/docs/SKILL.md:12: the `docs` skill's file differs from sourceorigin-standard's copy: {sync}"
            ),
            ".agents/skills/docs/notes.md:1: the `docs` skill doesn't ship this file, and git \
             doesn't track it, so `--fix` leaves it: move it out of the synced skill, or delete \
             it (skills-synced)"
                .to_owned(),
            format!(
                ".agents/skills/old/SKILL.md:1: sourceorigin-standard no longer ships the `old` skill: {sync}"
            ),
            format!(
                ".agents/skills/old/scripts/run.py:1: sourceorigin-standard no longer ships the `old` skill: {sync}"
            ),
        ]
    );

    fix(fixture.0.root()).expect("the fixes are made");
    assert_eq!(fixture.violations(Rule::SkillsSynced).len(), 1);
    let skills = fixture.0.root().join(DIR);
    assert!(!skills.join("old").exists());
    assert!(skills.join("docs/notes.md").exists());
    assert!(skills.join("repo-dev/SKILL.md").exists());
    assert!(skills.join("docs/.SKILL.md.swp").exists());
}

#[test]
fn a_directory_of_the_repositorys_own_named_for_a_skill_is_reported_and_left_alone() {
    let fixture = Fixture::new();
    fixture.add_crate("core", INHERITS);
    std::fs::remove_dir_all(fixture.0.root().join(DIR).join("docs")).expect("the skill is removed");
    fixture
        .write(
            ".agents/skills/docs/SKILL.md",
            "---\nname: docs\n---\n\nOurs.\n",
        )
        .write(".agents/skills/docs/book.md", "ours\n");
    assert_eq!(
        fixture.violations(Rule::SkillsSynced),
        [
            ".agents/skills/docs/SKILL.md:1: `.agents/skills/docs` is this repository's own \
             skill, as its `SKILL.md` doesn't say it is synced, but sourceorigin-standard ships \
             a `docs` skill: rename the directory, or delete it for `cargo xtask standard \
             skills` to write the crate's (skills-synced)"
        ]
    );

    fix(fixture.0.root()).expect("the fixes are made");
    let read = |path: &str| {
        std::fs::read_to_string(fixture.0.root().join(DIR).join(path)).expect("the file is kept")
    };
    assert_eq!(read("docs/SKILL.md"), "---\nname: docs\n---\n\nOurs.\n");
    assert_eq!(read("docs/book.md"), "ours\n");
}

#[test]
fn an_old_synced_copy_under_claude_skills_is_deleted_rather_than_moved_but_untracked_files_stay() {
    let fixture = Fixture::new();
    fixture.add_crate("core", INHERITS);
    std::fs::remove_dir_all(fixture.0.root().join(DIR)).expect("the skills are removed");
    std::fs::remove_file(fixture.0.root().join(".claude/skills")).ok();
    fixture
        .write(
            ".claude/skills/standard/SKILL.md",
            &format!("---\nname: standard\n---\n\n{MARKER}\nOld.\n"),
        )
        .write(
            ".claude/skills/mine/SKILL.md",
            "---\nname: mine\n---\nMine.\n",
        );
    let added = std::process::Command::new("git")
        .args(["add", ".claude/skills/standard/SKILL.md"])
        .current_dir(fixture.0.root())
        .status()
        .expect("git runs");
    assert!(added.success(), "git tracks the old copy");
    fixture.write(".claude/skills/standard/my-notes.md", "Mine.\n");
    let fixes: Vec<String> = fix(fixture.0.root())
        .expect("the fixes are made")
        .iter()
        .map(ToString::to_string)
        .filter(|fix| fix.contains(".claude/skills"))
        .collect();
    assert_eq!(
        fixes,
        [
            ".claude/skills/mine/SKILL.md: move it to .agents/skills/mine/SKILL.md",
            ".claude/skills/standard/SKILL.md: delete the old copy of the `standard` skill",
        ]
    );
    let skills = fixture.0.root().join(DIR);
    assert!(skills.join("standard/SKILL.md").exists() && skills.join("mine/SKILL.md").exists());
    assert!(
        fixture
            .0
            .root()
            .join(".claude/skills/standard/my-notes.md")
            .exists(),
        "an untracked file is left"
    );
}

#[test]
fn an_old_synced_copy_with_uncommitted_changes_is_reported_and_kept() {
    let fixture = Fixture::new();
    fixture.add_crate("core", INHERITS);
    std::fs::remove_dir_all(fixture.0.root().join(DIR)).expect("the skills are removed");
    std::fs::remove_file(fixture.0.root().join(".claude/skills")).ok();
    let old = ".claude/skills/standard/SKILL.md";
    fixture.write(
        old,
        &format!("---\nname: standard\n---\n\n{MARKER}\nOld.\n"),
    );
    let added = std::process::Command::new("git")
        .args(["add", old])
        .current_dir(fixture.0.root())
        .status()
        .expect("git runs");
    assert!(added.success(), "git tracks the old copy");
    fixture.write(
        old,
        &format!("---\nname: standard\n---\n\n{MARKER}\nEdited.\n"),
    );
    let fixes: Vec<String> = fix(fixture.0.root())
        .expect("the fixes are made")
        .iter()
        .map(ToString::to_string)
        .filter(|fix| fix.contains(".claude/skills"))
        .collect();
    assert_eq!(fixes, Vec::<String>::new());
    assert!(
        fixture.0.root().join(old).exists(),
        "the edited copy is kept"
    );
    assert!(
        fixture
            .violations(Rule::AgentFiles)
            .iter()
            .any(|violation| violation.contains("changes git hasn't committed")),
        "it is reported"
    );
}
