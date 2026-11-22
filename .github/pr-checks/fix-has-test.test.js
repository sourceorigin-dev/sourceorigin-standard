'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const fixHasTest = require('./fix-has-test.js');
const { Fake } = require('./testing.js');

const file = (filename, patch = '', status = 'modified') => ({ filename, patch, status });
const findings = async (pr, changed = []) => fixHasTest.findings(await Fake.pr(pr, { changed }));

test('a pull request of another type passes without a test', async () => {
  assert.deepEqual(await findings({ title: 'feat(rules): a' }, [file('src/lib.rs')]), []);
});

test('a fix changing a test file passes', async () => {
  for (const path of ['crates/a/src/rules/tests.rs', 'tests/cli.rs', 'web/src/a.test.ts', 'b_test.go', 'test_c.py']) {
    assert.deepEqual(await findings({ title: 'fix: a' }, [file('src/lib.rs'), file(path)]), [], path);
  }
});

test('a fix adding a test beside the code passes', async () => {
  for (const attribute of ['#[test]', '#[tokio::test]', '#[rstest]', '#[test_case(1)]']) {
    const patch = `@@ -1,1 +1,3 @@\n fn a() {}\n+    ${attribute}\n+fn b() {}`;
    assert.deepEqual(await findings({ title: 'fix: a' }, [file('src/lib.rs', patch)]), [], attribute);
  }
});

test('a fix editing an existing inline test module passes, one editing only the code not', async () => {
  const source = ['fn a() {}', '', '#[cfg(test)]', 'mod tests {', '    fn helper() {', '        a();', '    }', '}', ''].join('\n');
  const pr = async (patch) =>
    fixHasTest.findings(await Fake.pr({ title: 'fix: a' }, { changed: [file('src/lib.rs', patch)], files: { head: { 'src/lib.rs': source } } }));
  assert.deepEqual(await pr('@@ -5,2 +5,2 @@ mod tests {\n     fn helper() {\n-        b();\n+        a();'), []);
  assert.equal((await pr('@@ -1 +1 @@\n-fn b() {}\n+fn a() {}')).length, 1);
  assert.deepEqual(fixHasTest.testModules(source), [[4, 8]]);
});

test('a fix without a test fails with how to add one', async () => {
  const removed = file('src/tests.rs', '', 'removed');
  const unchanged = file('src/lib.rs', '@@ -1 +1 @@\n-#[test]\n+fn b() {}');
  const [finding] = await findings({ title: 'fix(rules)!: a' }, [removed, unchanged]);
  assert.match(finding.message, /^this `fix` changes no test: add one that fails without the fix/);
});

test('a visual fix passes with an image and fails without one', async () => {
  const labels = [{ name: 'visual' }];
  for (const body of ['![before](https://x/a.png)', '<img src="b.png">', 'https://github.com/user-attachments/assets/1']) {
    assert.deepEqual(await findings({ title: 'fix: a', labels, body }), [], body);
  }
  const [finding] = await findings({ title: 'fix: a', labels, body: 'Looks better.' });
  assert.match(finding.message, /the description has no image/);
});

test('a pull request whose files GitHub lists only in part fails closed', async () => {
  const pr = await Fake.pr({ title: 'fix: a', changed_files: 3001 }, { changed: [file('tests/a.rs')] });
  await assert.rejects(fixHasTest.findings(pr), /GitHub lists 1 of the 3001 files/);
});
