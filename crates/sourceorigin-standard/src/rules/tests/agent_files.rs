use super::{Fixture, INHERITS};
use crate::rules::Rule;

/// A skill's file, its frontmatter naming it `name`.
fn skill(name: &str) -> String {
    format!("---\nname: {name}\ndescription: Does a thing.\n---\n\n# Steps\n")
}

#[test]
fn agents_read_agents_md_and_skills_under_agents() {
    let fixture = Fixture::new();
    fixture
        .add_crate("core", INHERITS)
        .write("crates/core/CLAUDE.md", "@AGENTS.md\n")
        .write(".agents/skills/review/SKILL.md", &skill("review"))
        .write(".agents/skills/deploy/SKILL.md", &skill("ship"))
        .write(".claude/skills/debug/SKILL.md", &skill("debug"));
    assert_eq!(
        fixture.violations(Rule::AgentFiles),
        [
            ".agents/skills/deploy/SKILL.md:1: the skill's frontmatter names it `ship`: start \
             the file with `---`, `name: deploy`, `---` (agent-files)",
            ".claude/skills:1: .claude/skills isn't a symlink to `../.agents/skills`: make it \
             one, with `ln -s ../.agents/skills .claude/skills` (agent-files)",
            ".claude/skills/debug/SKILL.md:1: a skill under .claude/skills: move it to \
             .agents/skills (agent-files)",
            "AGENTS.md:1: no AGENTS.md: write the agents' instructions there (agent-files)",
            "crates/core/CLAUDE.md:1: a CLAUDE.md: move what it says to AGENTS.md, which Claude \
             Code reads too, and delete it (agent-files)",
        ]
    );
}

#[cfg(unix)]
#[test]
fn claude_code_reads_the_skills_through_a_symlink() {
    let fixture = Fixture::new();
    fixture
        .add_crate("core", INHERITS)
        .write("AGENTS.md", "# Repository\n")
        .write(".agents/skills/review/SKILL.md", &skill("review"));
    let claude = fixture.0.root().join(".claude");
    std::fs::create_dir(&claude).expect("the directory is created");
    std::os::unix::fs::symlink("../.agents/skills", claude.join("skills"))
        .expect("the symlink is created");
    assert_eq!(fixture.violations(Rule::AgentFiles), Vec::<String>::new());
}
