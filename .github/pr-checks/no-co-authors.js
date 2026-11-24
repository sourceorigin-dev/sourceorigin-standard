'use strict';

/** A line that credits a co-author, in any case. */
const TRAILER = /^\s*co-authored-by:.*$/im;

/**
 * G4: neither the description nor any commit message carries a
 * `Co-authored-by` trailer. The description counts as written, comments
 * included, as a squash merge may copy it into the commit on `main`.
 */
const noCoAuthors = {
  name: 'no-co-authors',

  async findings(pr) {
    const findings = [];
    const body = TRAILER.exec(pr.body);
    if (body) {
      findings.push({
        message: `the description credits a co-author with \`${body[0].trim()}\`: delete that line.`,
      });
    }
    for (const { sha, message } of await pr.commits()) {
      const trailer = TRAILER.exec(message);
      if (!trailer) continue;
      findings.push({
        message:
          `commit ${sha.slice(0, 7)} ("${message.split('\n')[0]}") credits a co-author with ` +
          `\`${trailer[0].trim()}\`: reword it without that line (\`git rebase -i\`), then push ` +
          `with \`--force-with-lease\`.`,
      });
    }
    return findings;
  },
};

module.exports = noCoAuthors;
