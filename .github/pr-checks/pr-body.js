'use strict';

/** Where the repository's pull request template lives. */
const TEMPLATE = '.github/pull_request_template.md';

/** The words that make GitHub or Linear close an issue a description names. */
const CLOSING = ['close', 'closes', 'closed', 'fix', 'fixes', 'fixed', 'resolve', 'resolves', 'resolved'];

/**
 * G3 and G7: the description has every heading of the repository's pull
 * request template, each with something under it besides the template's
 * comments. An issue link is optional; one that closes an issue is
 * `Fixes <prefix><number>`, in the form the repository's issues take.
 */
const prBody = {
  name: 'pr-body',

  async findings(pr, settings) {
    const template = await pr.fileAt(pr.base, TEMPLATE);
    if (template === null) {
      return [
        {
          path: TEMPLATE,
          message: `the repository has no pull request template, whose headings a description follows: add one with \`## What and why\` and \`## Testing\``,
        },
      ];
    }
    const missing = prBody.missing(template, pr.body).map((heading) => ({
      message:
        `the description has nothing under \`${heading}\`: fill in every heading of ${TEMPLATE}, ` +
        `saying what was wrong, what changes and how it was tested, including what wasn't.`,
    }));
    const links = prBody.wrongLinks(pr.body, settings.issuePrefix).map((link) => ({
      message:
        `\`${link}\` closes an issue in another form than this repository's: write ` +
        `\`Fixes ${settings.issuePrefix}<number>\` to close it, or \`Part of ${settings.issuePrefix}<number>\` ` +
        `to only link it.`,
    }));
    return [...missing, ...links];
  },

  /** The template's headings that `body` lacks or leaves empty. */
  missing(template, body) {
    const sections = prBody.sections(body);
    return prBody
      .sections(template)
      .map(({ heading }) => heading)
      .filter(
        (heading) =>
          !sections.some(
            (section) => section.heading.toLowerCase() === heading.toLowerCase() && section.text,
          ),
      );
  },

  /** Each heading of `markdown` and the text under it, its subsections'
   * included, without HTML comments or lines in code fences. */
  sections(markdown) {
    const sections = [];
    let fenced = false;
    for (const line of prBody.prose(markdown).split(/\r?\n/)) {
      if (/^\s*(```|~~~)/.test(line)) fenced = !fenced;
      const heading = !fenced && /^(#{1,6})\s+(.+?)[\s#]*$/.exec(line);
      if (heading) sections.push({ level: heading[1].length, heading: heading[2], text: '' });
      else if (sections.length) sections.at(-1).text += line.trim();
    }
    return sections.map((section, index) => {
      let { text } = section;
      for (const next of sections.slice(index + 1)) {
        if (next.level <= section.level) break;
        text += next.heading + next.text;
      }
      return { heading: section.heading, text };
    });
  },

  /** `markdown` without its HTML comments, which hold a template's prompts. */
  prose(markdown) {
    return markdown.replace(/<!--[\s\S]*?(-->|$)/g, '');
  },

  /** The closing references in `body` that aren't `Fixes <prefix><n>`.
   * A reference is an issue in GitHub's form or the repository's, so
   * `fixes UTF-8` or `resolves CVE-2024-1` aren't; those in code, which
   * closes nothing, don't count. */
  wrongLinks(body, prefix) {
    const escaped = prefix.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
    const issues = ['#\\d+', '[\\w.-]+/[\\w.-]+#\\d+', 'https://github\\.com/[\\w.-]+/[\\w.-]+/issues/\\d+'];
    if (prefix !== '#') issues.push(`${escaped}\\d+`);
    const reference = new RegExp(`\\b([A-Za-z]+):?\\s+(${issues.join('|')})(?![\\w-])`, 'g');
    const form = new RegExp(`^${escaped}\\d+$`);
    const text = prBody.prose(body).replace(/^\s*(```|~~~)[\s\S]*?^\s*\1/gm, '').replace(/`[^`\n]*`/g, '');
    return [...text.matchAll(reference)]
      .filter(([, keyword]) => CLOSING.includes(keyword.toLowerCase()))
      .filter(([, keyword, issue]) => keyword.toLowerCase() !== 'fixes' || !form.test(issue))
      .map(([link]) => link.replace(/\s+/g, ' '));
  },
};

module.exports = prBody;
