'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const { Settings } = require('./settings.js');

test('a missing file or table is the defaults', () => {
  for (const text of [null, '[comments]\ndoc-lines = 10']) {
    const settings = new Settings(text, ['pr-title']);
    assert.deepEqual([settings.scopes, settings.deviations.size], [[], 0]);
  }
});

test('the [pr-checks] table and its deviations read', () => {
  const settings = new Settings(
    '[deviations]\nbudget = "other tables are the crate\'s"\n\n[pr-checks]\nscopes = ["rules", "xtask"]\n\n' +
      '[pr-checks.deviations]\npr-title = "why"',
    ['pr-title'],
  );
  assert.deepEqual(settings.scopes, ['rules', 'xtask']);
  assert.deepEqual([...settings.deviations], [['pr-title', 'why']]);
});

test('an unknown key or a wrong value fails, naming its line', () => {
  for (const [text, message] of [
    ['[pr-checks]\nscope = ["a"]', /standard\.toml:2: `scope` of `\[pr-checks\]` must be one of scopes/],
    ['[pr-checks]\nscopes = "a"', /standard\.toml:2: `scopes` .* must be an array of strings/],
    ['[pr-checks]\nscopes = [1]', /must be an array of strings/],
    ['[pr-checks.deviations]\npr-size = "big"', /`pr-size` .* must be one of pr-title/],
    ['[pr-checks.deviations]\npr-title = ""', /`pr-title` .* must be one of pr-title, given as/],
  ]) {
    assert.throws(() => new Settings(text, ['pr-title']), message, text);
  }
});

test('booleans and strings read as their kind', () => {
  assert.deepEqual(
    ['true', 'false', '"x"'].map((value) => Settings.read('boolean', value)),
    [true, false, null],
  );
  assert.equal(Settings.read('string', '"PQD-"'), 'PQD-');
  assert.equal(Settings.property('enforce-lint-exceptions'), 'enforceLintExceptions');
});

test('a form of pr-checks the checks would not read fails, naming its line', () => {
  for (const text of [
    'pr-checks = { scopes = ["a"] }',
    'pr-checks.scopes = ["a"]',
    '[pr-checks]\ndeviations.pr-title = "x"',
    '[pr-checks.other]\nx = 1',
  ]) {
    assert.throws(() => new Settings(text, ['pr-title']), /standard\.toml:\d: write `pr-checks/, text);
  }
});
