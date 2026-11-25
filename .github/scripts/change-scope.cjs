const { execFileSync } = require('node:child_process');

// Repository settings that no build, test or workflow reads.
const SETTINGS = ['.github/dependabot.yml', '.github/CODEOWNERS', '.gitignore'];

function isDocumentation(path) {
  return path.endsWith('.md') || path.startsWith('docs/') || /^LICENSE[^/]*$/.test(path)
    || SETTINGS.includes(path) || path.startsWith('.github/ISSUE_TEMPLATE/');
}

function detect(context, warn, git = execFileSync) {
  const event = context.payload;
  if (!['pull_request', 'push'].includes(context.eventName)) return true;
  const base = event.pull_request?.base.sha ?? event.before;
  const head = event.pull_request?.head.sha ?? event.after;
  if (!base || /^0+$/.test(base) || !head) return true;
  try {
    // Disable rename detection: a code file renamed into docs still needs checks.
    // PRs compare against the merge base; pushes compare their exact before/after.
    const range = context.eventName === 'pull_request' ? `${base}...${head}` : `${base}..${head}`;
    const paths = git('git', ['diff', '--name-only', '--no-renames', '-z', range, '--'], {
      encoding: 'utf8', maxBuffer: 16 * 1024 * 1024,
    }).split('\0').filter(Boolean);
    return paths.some(path => !isDocumentation(path));
  } catch (error) {
    // A missing history object must never silently bypass required checks.
    warn(`Could not determine changed files; running full checks: ${error.message}`);
    return true;
  }
}

module.exports = detect;
module.exports.isDocumentation = isDocumentation;
