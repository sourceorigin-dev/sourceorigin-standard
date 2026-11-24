'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const noCoAuthors = require('./no-co-authors.js');
const { Fake } = require('./testing.js');

const commit = (sha, message) => ({ sha, commit: { message } });

test('a pull request with no co-author passes', async () => {
  const commits = [commit('a'.repeat(40), 'feat: a\n\nMentions co-authored-by in prose.')];
  assert.deepEqual(await noCoAuthors.findings(await Fake.pr({ body: 'Body.' }, { commits })), []);
});

test('a trailer in the description fails, in any case and inside a comment', async () => {
  for (const body of ['x\n\nCo-Authored-By: A <a@b>', '<!--\nco-authored-by: A <a@b>\n-->']) {
    const [finding] = await noCoAuthors.findings(await Fake.pr({ body }));
    assert.match(finding.message, /^the description credits a co-author with `co-authored-by: A <a@b>`/i);
  }
});

test('each commit with a trailer fails, named by its short SHA and subject', async () => {
  const commits = [
    commit('1'.repeat(40), 'feat: a\n\nCo-authored-by: A <a@b>'),
    commit('2'.repeat(40), 'fix: b'),
    commit('3'.repeat(40), 'docs: c\n\nCo-authored-by: C <c@d>'),
  ];
  const findings = await noCoAuthors.findings(await Fake.pr({}, { commits }));
  assert.deepEqual(
    findings.map(({ message }) => message.split(' credits')[0]),
    ['commit 1111111 ("feat: a")', 'commit 3333333 ("docs: c")'],
  );
});
