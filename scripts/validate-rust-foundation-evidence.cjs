const fs = require('node:fs');
const path = require('node:path');

const PARTITIONS = ['prerequisites', 'core', 'runtime', 'adapters'];

function fail(message) {
  throw new Error(message);
}

function regularFile(file, label) {
  const stat = fs.lstatSync(file);
  if (!stat.isFile() || stat.isSymbolicLink()) fail(`${label} must be a regular file`);
}

function safeDirectory(dir, label) {
  const stat = fs.lstatSync(dir);
  if (!stat.isDirectory() || stat.isSymbolicLink()) fail(`${label} must be a directory`);
}

function readRegular(file, label) {
  regularFile(file, label);
  return fs.readFileSync(file);
}

function requiredEnv(env, key) {
  const value = env[key];
  if (typeof value !== 'string' || value.length === 0) fail(`missing ${key}`);
  return value;
}

function attempt(value, max, label) {
  if (typeof value !== 'string' || !/^[1-9][0-9]*$/.test(value)) {
    fail(`${label} run attempt is invalid`);
  }
  const parsed = Number(value);
  if (!Number.isSafeInteger(parsed) || parsed > max) {
    fail(`${label} run attempt is outside the current workflow run`);
  }
  return parsed;
}

function validate(root, env = process.env) {
  const expected = {
    source_revision: requiredEnv(env, 'GITHUB_SHA'),
    run_id: requiredEnv(env, 'GITHUB_RUN_ID'),
    target: requiredEnv(env, 'TARGET_TRIPLE'),
    runner_arch: requiredEnv(env, 'RUNNER_ARCH'),
  };
  const currentAttempt = Number(requiredEnv(env, 'GITHUB_RUN_ATTEMPT'));
  if (!Number.isSafeInteger(currentAttempt) || currentAttempt < 1) {
    fail('current workflow run attempt is invalid');
  }

  const partitionsRoot = path.join(root, 'partitions');
  safeDirectory(partitionsRoot, 'partition root');
  const observedPartitions = fs.readdirSync(partitionsRoot).sort();
  const expectedPartitions = [...PARTITIONS].sort();
  if (observedPartitions.length !== PARTITIONS.length
    || observedPartitions.some((name, index) => name !== expectedPartitions[index])) {
    fail('partition set is incomplete or contains unexpected entries');
  }

  for (const partition of PARTITIONS) {
    const dir = path.join(partitionsRoot, partition);
    safeDirectory(dir, `${partition} partition`);
    const file = path.join(dir, 'identity.json');
    const identity = JSON.parse(readRegular(file, `${partition} identity`).toString('utf8'));
    if (identity.partition !== partition
      || identity.source_revision !== expected.source_revision
      || identity.target !== expected.target
      || identity.runner_arch !== expected.runner_arch
      || identity.run_id !== expected.run_id) {
      fail(`${partition} partition identity mismatch`);
    }
    attempt(identity.run_attempt, currentAttempt, partition);
  }

  const controlsDir = path.join(root, 'A10-linux-control');
  safeDirectory(controlsDir, 'semantic-control root');
  const entries = fs.readdirSync(controlsDir);
  const prefix = `${expected.run_id}-`;
  const matches = entries.filter((name) => name.startsWith(prefix));
  if (matches.length !== 1) fail('expected exactly one same-run semantic-control receipt');
  const match = /^([0-9]+)-([1-9][0-9]*)-([A-Z0-9]+)\.([A-Za-z0-9]{8})$/.exec(matches[0]);
  if (!match || match[1] !== expected.run_id || match[3] !== expected.runner_arch) {
    fail('semantic-control path identity mismatch');
  }
  attempt(match[2], currentAttempt, 'semantic-control');

  const controlDir = path.join(controlsDir, matches[0]);
  safeDirectory(controlDir, 'semantic-control receipt');
  const identityFile = path.join(controlDir, 'identity.txt');
  const identityBytes = readRegular(identityFile, 'semantic-control identity');
  const expectedIdentity = Buffer.from(
    `head=${expected.source_revision}\nrunner_arch=${expected.runner_arch}\nrun_id=${expected.run_id}\nrun_attempt=${match[2]}\n`,
    'utf8',
  );
  // The producer appends uname and rustc provenance after the bound identity header.
  if (!identityBytes.subarray(0, expectedIdentity.length).equals(expectedIdentity)) {
    fail('semantic-control identity mismatch');
  }
  const result = readRegular(path.join(controlDir, 'control-result.txt'), 'semantic-control result');
  if (!result.equals(Buffer.from('EXPECTED_SEMANTIC_FAILURE_CONFIRMED\n', 'utf8'))) {
    fail('semantic-control result mismatch');
  }
}

module.exports = { validate };

if (require.main === module) {
  try {
    validate(process.argv[2] || 'target/rust-evidence');
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}
