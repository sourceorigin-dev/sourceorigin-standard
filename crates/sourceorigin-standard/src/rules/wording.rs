//! `wording`: the docs name things as the repository does.

use super::fix::{Checked, Fix};
use super::{Finding, Repository};

impl Repository {
    /// Each line of a Markdown file holding a spelling `[wording]` rejects
    /// as a word, outside code, fixed by writing the preferred one there.
    pub(super) fn wording(&self) -> Checked {
        let holds = |text: &str, word: &str| {
            text.match_indices(word).any(|(at, _)| {
                let before = text[..at].chars().next_back();
                let after = text[at + word.len()..].chars().next();
                !before.is_some_and(char::is_alphanumeric)
                    && !after.is_some_and(char::is_alphanumeric)
            })
        };
        let mut checked = Checked::default();
        for file in &self.markdown {
            for (line, prose) in file.prose_lines() {
                let rejected: Vec<_> = self
                    .config
                    .wording
                    .iter()
                    .filter(|(rejected, _)| holds(&prose, &rejected.value))
                    .collect();
                if rejected.is_empty() {
                    continue;
                }
                let original = file.text.lines().nth(line - 1).unwrap_or_default();
                let mut fixed: Vec<String> = original.split('`').map(str::to_owned).collect();
                for (rejected, preferred) in rejected {
                    checked.findings.push(Finding {
                        path: file.path.clone(),
                        line,
                        message: format!("write `{preferred}`, not `{}`", rejected.value),
                    });
                    for outside_code in fixed.iter_mut().step_by(2) {
                        *outside_code = outside_code.replace(&rejected.value, preferred);
                    }
                }
                checked.fixes.push(Fix::line(
                    file.path.clone(),
                    format!("write the names `[wording]` prefers on line {line}"),
                    line,
                    fixed.join("`"),
                ));
            }
        }
        checked
    }
}
