import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { REQUIRED_PARTITIONS, validateControlEvidence, validateFoundationEvidence, validatePartitionIdentities } from '../foundation-aggregation.mjs';

const expected = Object.freeze({ sourceRevision: '0123456789abcdef', target: 'x86_64-unknown-linux-gnu', runId: '41515893105', runAttempt: 2, runnerArch: 'X64' });
const partitions = (attempt = '1') => REQUIRED_PARTITIONS.map((partition) => ({
  partition,
  source_revision: expected.sourceRevision,
  target: expected.target,
  runner_arch: expected.runnerArch,
  run_id: expected.runId,
  run_attempt: attempt,
}));
const control = (attempt = '1', overrides = {}) => [{
  directoryName: `${expected.runId}-${attempt}-${expected.runnerArch}.abc12345`,
  identity: {
    source_revision: expected.sourceRevision,
    target: expected.target,
    runner_arch: expected.runnerArch,
    run_id: expected.runId,
    run_attempt: attempt,
    ...overrides,
  },
  result: 'EXPECTED_SEMANTIC_FAILURE_CONFIRMED\n',
  regularFiles: true,
}];

test('accepts a clean first-attempt Foundation run', () => {
  const firstAttempt = { ...expected, runAttempt: 1 };
  const identities = partitions('1');
  const attempts = validatePartitionIdentities(identities, firstAttempt);
  assert.deepEqual([...attempts.values()], [1, 1, 1, 1]);
  assert.equal(validateControlEvidence(control('1'), firstAttempt, 1), 1);
});

test('accepts failed-job-only reruns with authenticated retained partitions from the same run', () => {
  const identities = partitions('1');
  identities.find((identity) => identity.partition === 'core').run_attempt = '2';
  const attempts = validatePartitionIdentities(identities, expected);
  assert.deepEqual([...attempts.values()], [1, 2, 1, 1]);
  assert.equal(validateControlEvidence(control('1'), expected, attempts.get('prerequisites')), 1);
});

test('validates real evidence directories assembled from retained and rerun artifacts', () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'foundation-rerun-'));
  try {
    const partitionRoot = path.join(root, 'partitions');
    for (const identity of partitions()) {
      if (identity.partition === 'core') identity.run_attempt = '2';
      const directory = path.join(partitionRoot, identity.partition);
      fs.mkdirSync(directory, { recursive: true });
      fs.writeFileSync(path.join(directory, 'identity.json'), `${JSON.stringify(identity)}\n`);
    }
    const controlDirectory = path.join(root, 'A10-linux-control', `${expected.runId}-1-${expected.runnerArch}.abc12345`);
    fs.mkdirSync(controlDirectory, { recursive: true });
    fs.writeFileSync(path.join(controlDirectory, 'control-identity.json'), `${JSON.stringify(control('1')[0].identity)}\n`);
    fs.writeFileSync(path.join(controlDirectory, 'control-result.txt'), 'EXPECTED_SEMANTIC_FAILURE_CONFIRMED\n');
    const attempts = validateFoundationEvidence(root, expected);
    assert.deepEqual([...attempts.values()], [1, 2, 1, 1]);

    fs.symlinkSync(path.join(controlDirectory, 'control-result.txt'), path.join(controlDirectory, 'unexpected-link'));
    assert.throws(() => validateFoundationEvidence(root, expected), /unexpected or missing files/);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

test('rejects missing, duplicate, or unexpected partition identities', () => {
  assert.throws(() => validatePartitionIdentities(partitions().slice(1), expected), /missing partition/);
  assert.throws(() => validatePartitionIdentities([...partitions(), partitions()[0]], expected), /duplicate partition/);
  const wrong = partitions(); wrong[0].partition = 'unknown';
  assert.throws(() => validatePartitionIdentities(wrong, expected), /unexpected partition/);
});

test('rejects partition evidence from another source, run, target, architecture, or future attempt', () => {
  for (const [field, value, message] of [['source_revision', 'different', /source revision/], ['run_id', 'different', /workflow run/], ['target', 'different', /target/], ['runner_arch', 'ARM64', /architecture/], ['run_attempt', '3', /future workflow attempt/]]) {
    const changed = partitions(); changed[0][field] = value;
    assert.throws(() => validatePartitionIdentities(changed, expected), message);
  }
});

test('rejects invalid attempt values', () => {
  const changed = partitions(); changed[0].run_attempt = '0';
  assert.throws(() => validatePartitionIdentities(changed, expected), /positive decimal attempt/);
});

test('rejects missing, duplicate, mismatched, or altered semantic-control evidence', () => {
  assert.throws(() => validateControlEvidence([], expected, 1), /exactly one/);
  assert.throws(() => validateControlEvidence([...control('1'), ...control('1')], expected, 1), /exactly one/);
  for (const [field, value] of [['source_revision', 'different'], ['run_id', 'different'], ['target', 'different'], ['runner_arch', 'ARM64'], ['run_attempt', '2']]) {
    assert.throws(() => validateControlEvidence(control('1', { [field]: value }), expected, 1), /mismatch|differs/);
  }
  assert.throws(() => validateControlEvidence(control('1').map((entry) => ({ ...entry, result: 'EXPECTED_SEMANTIC_FAILURE_CONFIRMED\nextra\n' })), expected, 1), /altered/);
  assert.throws(() => validateControlEvidence(control('1').map((entry) => ({ ...entry, regularFiles: false })), expected, 1), /regular files/);
  assert.throws(() => validateControlEvidence(control('2'), expected, 1), /differs from its prerequisites/);
});

test('workflow downloads semantic-control evidence from any prior attempt of the same run', () => {
  const workflow = fs.readFileSync(new URL('../../.github/workflows/rust.yml', import.meta.url), 'utf8');
  assert.ok(workflow.includes('target/rust-evidence/A10-linux-control/${{ github.run_id }}-*-${{ runner.arch }}.*/'));
  assert.doesNotMatch(workflow, /A10-linux-control\/\$\{\{\s*github\.run_id\s*\}\}-\$\{\{\s*github\.run_attempt\s*\}\}-/);
});
