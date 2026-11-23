'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const lintExceptions = require('./lint-exceptions.js');
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

const pushed = [{ created_at: '2026-11-23T10:00:00Z', head_branch: 'feature' }, { created_at: '2026-11-23T10:00:05Z', head_branch: 'feature' }];
const labeled = (login, created_at) => ({ event: 'labeled', label: { name: 'approved-exception' }, actor: { login }, created_at });
const waiver = async ({ labels = [{ name: 'approved-exception' }], events = [], suites = pushed, settings = 'approvers = ["PiquelChips"]' } = {}) =>
  lintExceptions.waiver(await Fake.pr({ labels }, { events, suites }), Fake.settings(settings));

test("an approver's label after the latest push allows the exceptions, in any case", async () => {
  assert.equal(
    await waiver({ events: [labeled('piquelchips', '2026-11-23T10:01:00Z')] }),
    'piquelchips added the `approved-exception` label after the latest push',
  );
});

test("someone else's label, one before the push or a later force-push, or none does not", async () => {
  for (const options of [
    { events: [labeled('an-agent', '2026-11-23T10:01:00Z')] },
    { events: [labeled('PiquelChips', '2026-11-23T09:59:00Z')] },
    { events: [labeled('PiquelChips', '2026-11-23T09:00:00Z'), labeled('an-agent', '2026-11-23T10:01:00Z')] },
    { events: [labeled('PiquelChips', '2026-11-23T10:01:00Z')], suites: [] },
    {
      events: [labeled('PiquelChips', '2026-11-23T10:01:00Z')],
      suites: [{ created_at: '2026-11-23T09:00:00Z', head_branch: 'elsewhere' }],
    },
    { events: [labeled('PiquelChips', '2026-11-23T10:01:00Z')], labels: [] },
    {
      events: [
        labeled('PiquelChips', '2026-11-23T10:01:00Z'),
        { event: 'head_ref_force_pushed', actor: { login: 'an-agent' }, created_at: '2026-11-23T10:02:00Z' },
      ],
    },
  ]) {
    assert.equal(await waiver(options), undefined, JSON.stringify(options));
  }
});

test('report-only mode allows the exceptions', async () => {
  assert.match(await waiver({ labels: [], settings: 'enforce-lint-exceptions = false' }), /report-only mode/);
});
