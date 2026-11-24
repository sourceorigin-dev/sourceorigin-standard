'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const prTitle = require('./pr-title.js');

const scopes = ['rules', 'check-changed'];

test('a conventional title passes, with or without scope and breaking mark', () => {
  for (const title of ['feat: a', 'fix(rules): a', 'refactor(check-changed)!: a', 'docs!: a']) {
    assert.equal(prTitle.problem(title, scopes), null, title);
  }
});

test('a title without the shape fails', () => {
  for (const title of ['Add a thing', 'feat:a', 'feat(rules) : a', 'feat:  ', 'feat()x: a']) {
    assert.equal(prTitle.problem(title, scopes), 'is not `type(scope)!: subject`', title);
  }
});

test('a type outside the list fails', () => {
  assert.equal(prTitle.problem('feature: a', scopes), 'has the type `feature`');
  assert.equal(prTitle.problem('Feat: a', scopes), 'has the type `Feat`');
});

test('a scope the repository does not allow fails', () => {
  assert.equal(prTitle.problem('feat(web): a', scopes), 'has the scope `web`');
  assert.equal(prTitle.problem('feat(rules): a', []), 'has the scope `rules`');
});

test('the finding shows the expected form and an example in an allowed scope', async () => {
  const [finding] = await prTitle.findings({ title: 'Add a thing' }, { scopes });
  assert.match(finding.message, /`type\(scope\)!: subject`/);
  assert.match(finding.message, /one of rules, check-changed/);
  assert.match(finding.message, /`fix\(rules\): /);
});
