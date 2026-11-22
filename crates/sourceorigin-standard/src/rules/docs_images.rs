//! `docs-images`: every image in the docs says what it shows.

use super::text_file::TextFile;
use super::{Finding, Repository};

impl TextFile {
    /// The line of each image without alt text, outside code: `![](…)`, or
    /// an `<img>` without a non-empty `alt`.
    fn images_without_alt(&self) -> Vec<usize> {
        self.markdown_lines()
            .filter(|line| {
                let line = line.outside_code();
                line.contains("![](")
                    || line.match_indices("<img").any(|(at, _)| {
                        let tag = line[at..].split('>').next().unwrap_or_default();
                        !tag.contains("alt=") || tag.contains("alt=\"\"") || tag.contains("alt=''")
                    })
            })
            .map(|line| line.number)
            .collect()
    }
}

impl Repository {
    /// Each image in a Markdown file, but a synced skill's, without alt
    /// text.
    pub(super) fn docs_images(&self) -> Vec<Finding> {
        self.docs()
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
