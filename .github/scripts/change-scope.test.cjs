const { test } = require('node:test');
const assert = require('node:assert/strict');
const detect = require('./change-scope.cjs');

const push = { eventName: 'push', payload: { before: 'abc', after: 'def' } };
const classify = files => detect(push, assert.fail, () => files.join('\0') + '\0');

test('documentation paths skip checks; all other files run them', () => {
  assert.equal(classify(['README.md', 'docs/figure.png', 'docs/nested/design.md', 'LICENSE-MIT']), false);
  assert.equal(classify(['.github/dependabot.yml', '.github/CODEOWNERS', '.gitignore', '.github/ISSUE_TEMPLATE/bug.yml']), false);
  for (const path of ['crates/core/src/lib.rs', '.github/workflows/ci.yml', '.github/scripts/change-scope.cjs', 'ui/package.json', 'src/LICENSE', 'readme.MD', 'ui/.gitignore']) {
    assert.equal(classify(['README.md', path]), true, path);
  }
});

test('NUL separated filenames and deleted code paths are classified', () => {
  assert.equal(classify(['docs/a\nfigure.png']), false);
  assert.equal(classify(['src/a\nfile.rs', 'docs/renamed.md']), true);
});

test('PRs use merge base; pushes use exact range; renames remain two paths', () => {
  for (const [context, range] of [
    [push, 'abc..def'],
    [{ eventName: 'pull_request', payload: { pull_request: { base: { sha: 'abc' }, head: { sha: 'def' } } } }, 'abc...def'],
  ]) {
    detect(context, assert.fail, (command, args) => {
      assert.equal(command, 'git');
      assert.deepEqual(args, ['diff', '--name-only', '--no-renames', '-z', range, '--']);
      return 'README.md\0';
    });
  }
});

test('new branches, schedules, manual scans and unavailable history run checks', () => {
  const noGit = () => assert.fail('should not run git');
  assert.equal(detect({ eventName: 'schedule', payload: {} }, assert.fail, noGit), true);
  assert.equal(detect({ eventName: 'workflow_dispatch', payload: {} }, assert.fail, noGit), true);
  assert.equal(detect({ eventName: 'push', payload: { before: '0000', after: 'def' } }, assert.fail, noGit), true);
  let warned = false;
  assert.equal(detect(push, () => { warned = true; }, () => { throw new Error('missing object'); }), true);
  assert.equal(warned, true);
});
