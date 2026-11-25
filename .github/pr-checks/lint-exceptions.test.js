'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const lintExceptions = require('./lint-exceptions.js');
const { Settings } = require('./settings.js');
const { Fake } = require('./testing.js');

const file = (filename, status = 'modified', previous_filename) => ({ filename, status, previous_filename });
const lines = (findings) =>
  findings.map(({ path, line, message }) => `${path}:${line ?? ''} ${message.split(' A loosened rule')[0]}`);
const findings = async (changed, files = {}, settings = '') =>
  lines(await lintExceptions.findings(await Fake.pr({}, { changed, files }), Fake.settings(settings)));

test('any change to a rule file needs approval, unparsed', async () => {
  assert.deepEqual(
    await findings([file('clippy.toml'), file('deny.toml', 'removed'), file('crates/a/.clippy.toml', 'added'), file('src/lib.rs')]),
    [
      'clippy.toml: this changes clippy.toml, which sets how the rules apply.',
      'deny.toml: this deletes deny.toml, which sets how the rules apply.',
      'crates/a/.clippy.toml: this adds .clippy.toml, which sets how the rules apply.',
    ],
  );
});

test("a change to standard.toml needs approval, and its [pr-checks] must read", async () => {
  const head = { 'standard.toml': '[pr-checks]\nscopes = ["a"]' };
  assert.deepEqual(await findings([file('standard.toml')], { head }), [
    'standard.toml: this changes standard.toml, which sets how the rules apply.',
  ]);
  await assert.rejects(
    findings([file('standard.toml')], { head: { 'standard.toml': '[pr-checks]\nscope = 1' } }),
    /standard\.toml:2: `scope` of `\[pr-checks\]`/,
  );
});

const STANDARD = `unsafe-modules = ["a::ffi"]

[deviations]
doc-length = "long docs"
rust-comments = { reason = "vendored", paths = ["src/a.rs", "src/b.rs"], crates = ["a", "b"] }

[deviations.budget]
reason = "generated"
paths = ["budget.toml"]

[pr-checks]
scopes = ["a"]

[pr-checks.deviations]
pr-title = "a bot names its own"
`;

/** What changing STANDARD's `from` to `to` finds, or what a standard.toml
 * of `to` changed from `from` finds when `from` is a whole file. */
const changes = async (from, to, whole = false) => {
  const [base, head] = whole ? [from, to] : [STANDARD, STANDARD.replace(from, to)];
  assert.ok(whole || STANDARD.includes(from), from);
  const pr = await Fake.pr({}, { changed: [file('standard.toml')], files: { base: { 'standard.toml': base }, head: { 'standard.toml': head } } });
  return lines(await lintExceptions.findings(pr, new Settings('', ['pr-title', 'pr-body'])));
};
const loosens = ['standard.toml: this changes standard.toml, which sets how the rules apply.'];

test('a change that only tightens standard.toml needs no approval', async () => {
  for (const [from, to] of [
    ['doc-length = "long docs"\n', ''],
    ['[deviations.budget]\nreason = "generated"\npaths = ["budget.toml"]\n', ''],
    ['pr-title = "a bot names its own"\n', ''],
    ['paths = ["src/a.rs", "src/b.rs"]', 'paths = ["src/b.rs"]'],
    ['crates = ["a", "b"]', 'crates = []'],
    ['reason = "vendored"', 'reason = "vendored code, until #12 lands"'],
    ['"long docs"', '"generated docs"'],
    ['paths = ["budget.toml"]', 'paths = [\n  "budget.toml",\n]'],
  ]) {
    assert.deepEqual(await changes(from, to), [], `${from} -> ${to}`);
  }
});

test('a change to standard.toml that loosens it, or does anything else, needs approval', async () => {
  for (const [from, to] of [
    ['doc-length = "long docs"\n', 'doc-length = "long docs"\nwording = "quoted"\n'],
    ['paths = ["src/a.rs", "src/b.rs"]', 'paths = ["src/a.rs", "src/b.rs", "src/c.rs"]'],
    ['paths = ["src/a.rs", "src/b.rs"]', 'paths = ["src/a.rs", "src/c.rs"]'],
    ['doc-length = "long docs"\n', 'wording = "quoted"\n'],
    ['crates = ["a", "b"]', 'crates = ["a", "c"]'],
    ['[deviations.budget]\nreason = "generated"\npaths = ["budget.toml"]\n', '[deviations]\nbudget = "generated"\n'],
    ['doc-length = "long docs"', 'doc-length = { reason = "long docs", paths = ["src/a.rs"] }'],
    ['unsafe-modules = ["a::ffi"]', 'unsafe-modules = ["a::ffi", "b::ffi"]'],
    ['unsafe-modules = ["a::ffi"]', 'unsafe-modules = []'],
    ['unsafe-modules = ["a::ffi"]', 'unsafe-modules = ["a::ffi"]\nci-watchers = ["fail-fast"]'],
    ['scopes = ["a"]', 'scopes = ["a", "b"]'],
    ['pr-title = "a bot names its own"\n', 'pr-title = "a bot names its own"\npr-body = "too"\n'],
    ['[deviations]\n', '[deviations]\nnot a setting\n'],
  ]) {
    assert.deepEqual(await changes(from, to), loosens, `${from} -> ${to}`);
  }
  assert.deepEqual(await changes('', STANDARD, true), loosens);
  assert.deepEqual(await changes('[deviations]\nwording = "x"\nnot a setting\n', '', true), loosens);
  assert.deepEqual(
    await findings([file('standard.toml', 'added')], { head: { 'standard.toml': '' } }),
    ['standard.toml: this adds standard.toml, which sets how the rules apply.'],
  );
});

test("a manifest's allowed lints are budget.toml's to count, so only its raise needs approval", async () => {
  const base = { 'budget.toml': '[allow]\n"Cargo.toml" = 1\n' };
  const head = { 'budget.toml': '[allow]\n"Cargo.toml" = 2\n' };
  assert.deepEqual(await findings([file('Cargo.toml'), file('crates/a/Cargo.toml', 'added')]), []);
  assert.deepEqual(await findings([file('budget.toml')], { base, head }), [
    'budget.toml:2 this raises `allow.Cargo.toml` to 2.',
  ]);
});

test("Cargo's and Dylint's configuration and a rule file renamed away need approval", async () => {
  assert.deepEqual(
    await findings([file('.cargo/config.toml', 'added'), file('dylint.toml'), file('docs/old-clippy.txt', 'renamed', 'clippy.toml')]),
    [
      '.cargo/config.toml: this adds .cargo/config.toml, which sets how the rules apply.',
      'dylint.toml: this changes dylint.toml, which sets how the rules apply.',
      'docs/old-clippy.txt: this renames clippy.toml, which sets how the rules apply.',
    ],
  );
});

test('a budget ceiling that rises or appears needs approval, one that falls not', async () => {
  const base = { 'budget.toml': '[expect]\na = 3\nb = 2\n' };
  const head = { 'budget.toml': '[expect]\na = 4\nb = 1\n\n[invariant]\na = 1\n' };
  assert.deepEqual(await findings([file('budget.toml')], { base, head }), [
    'budget.toml:2 this raises `expect.a` to 4.',
    'budget.toml:6 this adds `invariant.a` to 1.',
  ]);
});

test('a budget.toml that is not tables of whole numbers fails closed', async () => {
  for (const text of ['a = 1', '[expect]\na = "3"', '[expect]\na = 1.5']) {
    const result = await findings([file('budget.toml')], { base: { 'budget.toml': '' }, head: { 'budget.toml': text } });
    assert.match(result[0], /^budget\.toml: this changes budget\.toml in a form other than tables of whole numbers/, text);
  }
});

test('each finding says who may approve it', async () => {
  const [finding] = await lintExceptions.findings(
    await Fake.pr({}, { changed: [file('deny.toml')] }),
    Fake.settings('approvers = ["PiquelChips"]'),
  );
  assert.match(finding.message, /needs the `approved-exception` label, added after the latest push by an approver .*: PiquelChips\)/);
});

const pushed = [Fake.push('2026-11-23T10:00:00Z')];
const labeled = (login, created_at) => ({ event: 'labeled', label: { name: 'approved-exception' }, actor: { login }, created_at });
const waiver = async ({ labels = [{ name: 'approved-exception' }], events = [], activity = pushed, settings = 'approvers = ["PiquelChips"]' } = {}) =>
  lintExceptions.waiver(await Fake.pr({ labels }, { events, activity }), Fake.settings(settings));
const approved = [labeled('PiquelChips', '2026-11-23T10:01:00Z')];

test("an approver's label after the latest push allows the exceptions, in any case", async () => {
  assert.equal(
    await waiver({ events: [labeled('piquelchips', '2026-11-23T10:01:00Z')] }),
    'piquelchips added the `approved-exception` label after the latest push',
  );
});

test("someone else's label, one before the latest push, or none does not", async () => {
  for (const options of [
    { events: [labeled('an-agent', '2026-11-23T10:01:00Z')] },
    { events: [labeled('PiquelChips', '2026-11-23T09:59:00Z')] },
    { events: [labeled('PiquelChips', '2026-11-23T09:00:00Z'), labeled('an-agent', '2026-11-23T10:01:00Z')] },
    { events: approved, labels: [] },
    { events: approved, activity: [...pushed, Fake.push('2026-11-23T10:02:00Z', { activity_type: 'force_push' })] },
  ]) {
    assert.equal(await waiver(options), undefined, JSON.stringify(options));
  }
});

test('only an entry moving the head branch of the head repository to the head commit dates the push', async () => {
  for (const other of [{ after: 'other' }, { ref: 'refs/heads/elsewhere' }, { repo: 'fork/r' }]) {
    const activity = [Fake.push('2026-11-23T09:00:00Z', other)];
    assert.equal(await waiver({ events: approved, activity }), undefined, JSON.stringify(other));
  }
  assert.equal(await waiver({ events: approved, activity: [] }), undefined);
  assert.equal(await waiver({ events: approved, activity: 403 }), undefined);
});

test('a commit pushed again after the label needs the label again (#69)', async () => {
  // C pushed at 10:00, a force-push back to A at 10:01, the label at 10:02,
  // then a fast-forward to C again at 10:03, which no workflow need run on,
  // as while the pull request conflicts.
  const activity = [
    Fake.push('2026-11-23T10:00:00Z'),
    Fake.push('2026-11-23T10:01:00Z', { activity_type: 'force_push', after: 'a' }),
    Fake.push('2026-11-23T10:03:00Z'),
  ];
  assert.equal(await waiver({ events: [labeled('PiquelChips', '2026-11-23T10:02:00Z')], activity }), undefined);
});

test("a log that doesn't hold the latest push yet approves nothing", async () => {
  // As #69's replay, but the fast-forward to C at 10:03 isn't logged yet:
  // the newest entry moved the branch to A.
  const activity = [
    Fake.push('2026-11-23T10:00:00Z'),
    Fake.push('2026-11-23T10:01:00Z', { activity_type: 'force_push', after: 'a' }),
  ];
  assert.equal(await waiver({ events: [labeled('PiquelChips', '2026-11-23T10:02:00Z')], activity }), undefined);
});

test('report-only mode allows the exceptions', async () => {
  assert.match(await waiver({ labels: [], settings: 'enforce-lint-exceptions = false' }), /report-only mode/);
});
