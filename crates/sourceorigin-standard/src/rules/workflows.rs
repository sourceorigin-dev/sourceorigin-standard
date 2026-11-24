//! The repository's GitHub Actions workflows and composite actions.

use std::path::{Path, PathBuf};

use super::Error;
use super::config_syntax::Unreadable;
use super::text_file::TextFile;
use super::yaml::Node;

/// The files, as git pathspecs.
const FILES: [&str; 4] = [
    ":(glob).github/workflows/*.yml",
    ":(glob).github/workflows/*.yaml",
    ":(glob).github/actions/**/action.yml",
    ":(glob).github/actions/**/action.yaml",
];

/// A workflow, or a composite action.
pub(super) struct Workflow {
    /// From the repository's root.
    pub(super) path: PathBuf,
    /// Its text, whose comments the document drops.
    pub(super) text: String,
    pub(super) document: Node,
}

impl Workflow {
    /// Every workflow and composite action under `root` that parses;
    /// `unreadable` records the others.
    pub(super) fn load_all(root: &Path, unreadable: &mut Unreadable) -> Result<Vec<Self>, Error> {
        let mut workflows = Vec::new();
        for file in TextFile::load_all(root, &FILES, unreadable)? {
            match Node::parse(&file.text) {
                Ok(Some(document)) => workflows.push(Self {
                    path: file.path,
                    text: file.text,
                    document,
                }),
                Ok(None) => {}
                Err(error) => {
                    unreadable.record(file.path, error.marker().line(), error.to_string());
                }
            }
        }
        Ok(workflows)
    }

    /// Whether it is a composite action rather than a workflow.
    pub(super) fn is_action(&self) -> bool {
        self.path.starts_with(".github/actions")
    }

    /// The events that trigger it, from `on` as a name, a list or a
    /// mapping.
    pub(super) fn triggers(&self) -> Vec<&str> {
        let Some(on) = self.document.get("on") else {
            return Vec::new();
        };
        on.as_str()
            .into_iter()
            .chain(on.items().iter().filter_map(Node::as_str))
            .chain(on.entries().iter().filter_map(|(event, _)| event.as_str()))
            .collect()
    }

    /// Its jobs, each with its key; none for an action.
    pub(super) fn jobs(&self) -> &[(Node, Node)] {
        self.document.get("jobs").map_or(&[], Node::entries)
    }

    /// The steps of every job, or of the action.
    pub(super) fn steps(&self) -> impl Iterator<Item = &Node> {
        let action = self
            .document
            .get("runs")
            .and_then(|runs| runs.get("steps"))
            .map_or(&[][..], Node::items);
        self.jobs()
            .iter()
            .flat_map(|(_, job)| job.get("steps").map_or(&[][..], Node::items))
            .chain(action)
    }
}
