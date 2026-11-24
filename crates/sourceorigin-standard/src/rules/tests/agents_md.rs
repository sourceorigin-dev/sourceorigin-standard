use super::{Fixture, INHERITS};
use crate::rules::Rule;

#[test]
fn agents_md_names_every_crate_briefly() {
    let fixture = Fixture::new();
    fixture
        .add_crate("core", INHERITS)
        .add_crate("store", INHERITS)
        .add_crate("cli", INHERITS)
        .write(
            "AGENTS.md",
            &format!(
                "# Repository\n\n- `core`: the model.\n- crates/store: the database.\n{}",
                "More.\n".repeat(150)
            ),
        );
    assert_eq!(
        fixture.violations(Rule::AgentsMd),
        [
            "AGENTS.md:1: AGENTS.md doesn't name the crate `cli`: add it to the map of the \
             workspace's crates, with a line saying what it is (agents-md)",
            "AGENTS.md:151: AGENTS.md holds 154 lines, over 150: keep what an agent can't infer \
             from the code, and move the rest to the docs it links (agents-md)",
        ]
    );
}
