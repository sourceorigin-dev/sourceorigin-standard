//! `docs-images`: every image in the docs says what it shows.

use super::text_file::TextFile;
use super::{Finding, Repository};

impl TextFile {
    /// The lines, from 1, outside fenced code blocks, without their inline
    /// code spans.
    pub(super) fn prose_lines(&self) -> impl Iterator<Item = (usize, String)> {
        let mut fenced = false;
        self.text
            .lines()
            .enumerate()
            .filter_map(move |(index, line)| {
                let fence =
                    line.trim_start().starts_with("```") || line.trim_start().starts_with("~~~");
                if fence {
                    fenced = !fenced;
                }
                let prose = line.split('`').step_by(2).collect();
                (!fence && !fenced).then_some((index + 1, prose))
            })
    }

    /// The line of each image without alt text: `![](…)`, or an `<img>`
    /// without a non-empty `alt`.
    fn images_without_alt(&self) -> Vec<usize> {
        self.prose_lines()
            .filter(|(_, line)| {
                line.contains("![](")
                    || line.match_indices("<img").any(|(at, _)| {
                        let tag = line[at..].split('>').next().unwrap_or_default();
                        !tag.contains("alt=") || tag.contains("alt=\"\"") || tag.contains("alt=''")
                    })
            })
            .map(|(line, _)| line)
            .collect()
    }
}

impl Repository {
    /// Each image in a Markdown file without alt text.
    pub(super) fn docs_images(&self) -> Vec<Finding> {
        self.markdown
            .iter()
            .flat_map(|file| {
                file.images_without_alt().into_iter().map(|line| Finding {
                    path: file.path.clone(),
                    line,
                    message: "an image without alt text: say what it shows, as `![what it \
                              shows](…)` or `alt=\"what it shows\"`"
                        .to_owned(),
                })
            })
            .collect()
    }
}
