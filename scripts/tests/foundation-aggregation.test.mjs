import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { REQUIRED_PARTITIONS, validateA13InventoryChecks, validateControlEvidence, validateCoreWorkspaceChecks, validateFoundationEvidence, validatePartitionIdentities } from '../foundation-aggregation.mjs';

const expected = Object.freeze({ sourceRevision: '0123456789abcdef', target: 'x86_64-unknown-linux-gnu', runId: '41515893105', runAttempt: 2, runnerArch: 'X64' });
const coreReport = () => ({
  plan_id: 'F03',
  status: 'passed',
  error: null,
  source: { head: expected.sourceRevision, diff: '', working_tree: '' },
  toolchain: `rustc 1.99.0 (fixture)\nhost: ${expected.target}\n`,
  commands: [
    ['fmt', '--all', '--check'],
    ['clippy', '--locked', '--workspace', '--all-targets', '--exclude', 'ssx-cli-hyper', '--', '-D', 'warnings'],
    ['clippy', '--locked', '-p', 'ssx-cli-hyper', '--features', 'full,source-client-framing', '--lib', '--', '-D', 'warnings'],
    ['test', '--locked', '--workspace', '--exclude', 'xtask', '--features', 'ssx-cli-hyper/full,ssx-cli-hyper/source-client-framing'],
  ].map((args) => ({ program: 'cargo', args, exit_code: 0 })),
});
const a13InventoryNames = [
  'source_inventory_is_closed_and_bound_without_original_execution',
  'hibp_static_source_inventory_closed',
  'intelx_static_source_inventory_closed',
];
const frozenA13 = JSON.parse(fs.readFileSync(new URL('../../docs/rust-rewrite/a13/qualification-v1.json', import.meta.url), 'utf8'));
const a13Report = () => ({
  ...coreReport(),
  plan_id: 'A13',
  commands: a13InventoryNames.flatMap((name) => {
    const selector = frozenA13.selectors.find((candidate) => candidate.name === name);
    return [
      { program: 'cargo', args: [...selector.argv], exit_code: 0, stdout: `test ${name} ... ok\n` },
      { A13_native_completion: name, parent_markers: [...selector.markers], original_execution: false, provider_calls: 0 },
    ];
  }),
});
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
  identityText: `head=${expected.sourceRevision}\nrunner_arch=${expected.runnerArch}\nrun_id=${expected.runId}\nrun_attempt=${attempt}\nLinux fixture x86_64 GNU/Linux\nrustc 1.99.0 (fixture)\n`,
  result: 'EXPECTED_SEMANTIC_FAILURE_CONFIRMED\n',
  regularFiles: true,
}];
test('replaces the stable aggregate artifact on a failed-job-only retry', () => {
  const workflow = fs.readFileSync(new URL('../../.github/workflows/rust.yml', import.meta.url), 'utf8');
  const marker = '      - name: Retain source-bound foundation evidence\n';
  const start = workflow.indexOf(marker);
  assert.notEqual(start, -1);
  const end = workflow.indexOf('\n      - name: ', start + marker.length);
  const step = workflow.slice(start, end === -1 ? undefined : end);
  assert.match(step, /name: rust-foundation-\$\{\{ matrix\.target \}\}-\$\{\{ github\.sha \}\}/);
  assert.match(step, /^\s+overwrite: true\s*$/m);
});

const controlProducerFiles = [
  'archive.sha256', 'control-identity.json', 'control-result.txt', 'copy-before-restore.sha256', 'copy-before.sha256z',
  'copy-restored.sha256', 'copy-restored.sha256z', 'identity.txt', 'live-after.sha256z', 'live-before.sha256z',
  'mutant.command', 'mutant.patch', 'mutant.session', 'mutant.status', 'mutant.stderr', 'mutant.stdout',
  'positive.command', 'positive.session', 'positive.status', 'positive.stderr', 'positive.stdout',
  'process-pool.before.rs', 'process-pool.mutant.rs', 'terminal.txt',
];

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
    fs.writeFileSync(path.join(controlDirectory, 'identity.txt'), control('1')[0].identityText);
    for (const name of controlProducerFiles.filter((name) => !['control-identity.json', 'control-result.txt', 'identity.txt'].includes(name))) {
      fs.writeFileSync(path.join(controlDirectory, name), 'producer evidence fixture\n');
    }
    assert.throws(() => validateFoundationEvidence(root, expected), /F03\.json/);
    fs.writeFileSync(path.join(root, 'F03.json'), JSON.stringify(coreReport()));
    assert.throws(() => validateFoundationEvidence(root, expected), /A13\.json/);
    fs.writeFileSync(path.join(root, 'A13.json'), JSON.stringify(a13Report()));
    const attempts = validateFoundationEvidence(root, expected);
    assert.deepEqual([...attempts.values()], [1, 2, 1, 1]);

    const prerequisitesIdentityPath = path.join(partitionRoot, 'prerequisites', 'identity.json');
    const coreIdentityPath = path.join(partitionRoot, 'core', 'identity.json');
    const prerequisitesIdentity = fs.readFileSync(prerequisitesIdentityPath, 'utf8');
    const coreIdentity = fs.readFileSync(coreIdentityPath, 'utf8');
    fs.writeFileSync(prerequisitesIdentityPath, coreIdentity);
    fs.writeFileSync(coreIdentityPath, prerequisitesIdentity);
    assert.throws(() => validateFoundationEvidence(root, expected), /does not match its directory/);
    fs.writeFileSync(prerequisitesIdentityPath, prerequisitesIdentity);
    fs.writeFileSync(coreIdentityPath, coreIdentity);

    fs.symlinkSync(path.join(controlDirectory, 'control-result.txt'), path.join(controlDirectory, 'unexpected-link'));
    assert.throws(() => validateFoundationEvidence(root, expected), /regular files only/);
    fs.unlinkSync(path.join(controlDirectory, 'unexpected-link'));
    fs.writeFileSync(path.join(controlDirectory, 'unexpected.txt'), 'unrecognized evidence\n');
    assert.throws(() => validateFoundationEvidence(root, expected), /unexpected or missing files/);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

test('requires all workspace coverage owned by F03 on the expected revision and architecture', () => {
  validateCoreWorkspaceChecks(coreReport(), expected);
  const wrongSource = coreReport(); wrongSource.source.head = 'different';
  assert.throws(() => validateCoreWorkspaceChecks(wrongSource, expected), /source mismatch/);
  const wrongTarget = coreReport(); wrongTarget.toolchain = 'host: aarch64-unknown-linux-gnu';
  assert.throws(() => validateCoreWorkspaceChecks(wrongTarget, expected), /target mismatch/);
  for (const index of [0, 1, 2, 3]) {
    const missing = coreReport(); missing.commands.splice(index, 1);
    assert.throws(() => validateCoreWorkspaceChecks(missing, expected), /one successful cargo/);
    const failed = coreReport(); failed.commands[index].exit_code = 1;
    assert.throws(() => validateCoreWorkspaceChecks(failed, expected), /one successful cargo/);
  }
  const narrowed = coreReport(); narrowed.commands[3].args.push('--bin', 'xtask');
  assert.throws(() => validateCoreWorkspaceChecks(narrowed, expected), /one successful cargo test/);
  const duplicated = coreReport(); duplicated.commands.push(duplicated.commands[3]);
  assert.throws(() => validateCoreWorkspaceChecks(duplicated, expected), /one successful cargo test/);
});

test('requires exact, successful A13 source-inventory completions on the expected revision and target', () => {
  validateA13InventoryChecks(a13Report(), expected);

  const wrongSource = a13Report(); wrongSource.source.head = 'different';
  assert.throws(() => validateA13InventoryChecks(wrongSource, expected), /source mismatch/);
  const wrongTarget = a13Report(); wrongTarget.toolchain = 'host: aarch64-unknown-linux-gnu';
  assert.throws(() => validateA13InventoryChecks(wrongTarget, expected), /target mismatch/);
  const failed = a13Report(); failed.status = 'failed';
  assert.throws(() => validateA13InventoryChecks(failed, expected), /did not pass/);
  const missing = a13Report(); missing.commands.splice(3, 1);
  assert.throws(() => validateA13InventoryChecks(missing, expected), /one validated completion/);
  const duplicate = a13Report(); duplicate.commands.push(duplicate.commands[1]);
  assert.throws(() => validateA13InventoryChecks(duplicate, expected), /one validated completion/);
  const duplicateCommand = a13Report(); duplicateCommand.commands.push({ ...duplicateCommand.commands[0] });
  assert.throws(() => validateA13InventoryChecks(duplicateCommand, expected), /exact inventory selector/);
  const noMarker = a13Report(); noMarker.commands[1].parent_markers = [];
  assert.throws(() => validateA13InventoryChecks(noMarker, expected), /completion is incomplete/);
  const wrongSuite = a13Report(); wrongSuite.commands[0].args[6] = 'a13_intelx';
  assert.throws(() => validateA13InventoryChecks(wrongSuite, expected), /exact inventory selector/);
  const ignored = a13Report(); ignored.commands[2].stdout = `test ${a13InventoryNames[1]} ... ignored\n`;
  assert.throws(() => validateA13InventoryChecks(ignored, expected), /exact inventory selector/);
  const failedCommand = a13Report(); failedCommand.commands[4].exit_code = 101;
  assert.throws(() => validateA13InventoryChecks(failedCommand, expected), /exact inventory selector/);
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
  assert.throws(() => validateControlEvidence(control('1').map((entry) => ({ ...entry, identityText: 'head=wrong\n' })), expected, 1), /text identity/);
  assert.throws(() => validateControlEvidence(control('1').map((entry) => ({ ...entry, regularFiles: false })), expected, 1), /regular files/);
  assert.throws(() => validateControlEvidence(control('2'), expected, 1), /differs from its prerequisites/);
});

test('workflow downloads semantic-control evidence from any prior attempt of the same run', () => {
  const workflow = fs.readFileSync(new URL('../../.github/workflows/rust.yml', import.meta.url), 'utf8');
  assert.ok(workflow.includes('target/rust-evidence/A10-linux-control/${{ github.run_id }}-*-${{ runner.arch }}.*/'));
  assert.doesNotMatch(workflow, /A10-linux-control\/\$\{\{\s*github\.run_id\s*\}\}-\$\{\{\s*github\.run_attempt\s*\}\}-/);
  const producer = workflow.split('name: Linux held-writer interpreter mutation control')[1].split('name: Locked workspace build')[0];
  assert.ok(producer);
  for (const name of controlProducerFiles) {
    const producerName = /^(?:positive|mutant)\.(?:command|session|status|stderr|stdout)$/.test(name)
      ? name.replace(/^(?:positive|mutant)\./, '$label.')
      : name;
    assert.ok(producer.includes(producerName), `A10 producer no longer writes ${name}`);
  }
});
