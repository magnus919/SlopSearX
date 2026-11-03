const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const test = require('node:test');
const yaml = fs.readFileSync('.github/workflows/rust.yml', 'utf8');
const { validate } = require('./validate-rust-foundation-evidence.cjs');

const current = {
  GITHUB_SHA: 'a'.repeat(40),
  GITHUB_RUN_ID: '12345',
  GITHUB_RUN_ATTEMPT: '2',
  RUNNER_ARCH: 'X64',
  TARGET_TRIPLE: 'x86_64-unknown-linux-gnu',
};
const partitions = ['prerequisites', 'core', 'runtime', 'adapters'];

test('Foundation upload retains same-run and same-architecture controls from prior attempts', () => {
  assert.match(
    yaml,
    /target\/rust-evidence\/A10-linux-control\/\$\{\{ github\.run_id \}\}-\*-\$\{\{ runner\.arch \}\}\.\*\//,
  );
  assert.doesNotMatch(
    yaml,
    /target\/rust-evidence\/A10-linux-control\/\$\{\{ github\.run_id \}\}-\$\{\{ github\.run_attempt \}\}-\$\{\{ runner\.arch \}\}/,
  );
});

function fixture({ partitionAttempt = '1', controlAttempt = '1', extraControls = [], mutate = () => {} } = {}) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'ssx-foundation-rerun-'));
  const evidence = path.join(root, 'rust-evidence');
  for (const partition of partitions) {
    const dir = path.join(evidence, 'partitions', partition);
    fs.mkdirSync(dir, { recursive: true });
    const identity = {
      partition,
      source_revision: current.GITHUB_SHA,
      target: current.TARGET_TRIPLE,
      runner_arch: current.RUNNER_ARCH,
      run_id: current.GITHUB_RUN_ID,
      run_attempt: partitionAttempt,
    };
    mutate(identity, partition);
    fs.writeFileSync(path.join(dir, 'identity.json'), JSON.stringify(identity));
  }
  const controls = path.join(evidence, 'A10-linux-control');
  fs.mkdirSync(controls, { recursive: true });
  for (const attempt of [controlAttempt, ...extraControls]) {
    const dir = path.join(controls, `${current.GITHUB_RUN_ID}-${attempt}-${current.RUNNER_ARCH}.AbCd1234`);
    fs.mkdirSync(dir);
    fs.writeFileSync(path.join(dir, 'identity.txt'),
      `head=${current.GITHUB_SHA}\nrunner_arch=${current.RUNNER_ARCH}\nrun_id=${current.GITHUB_RUN_ID}\nrun_attempt=${attempt}\nLinux test-kernel x86_64 GNU/Linux\nrustc 1.91.0 (test)\n`);
    fs.writeFileSync(path.join(dir, 'control-result.txt'), 'EXPECTED_SEMANTIC_FAILURE_CONFIRMED\n');
  }
  return { evidence, cleanup: () => fs.rmSync(root, { recursive: true, force: true }) };
}

test('accepts a clean current-attempt run', () => {
  const value = fixture({ partitionAttempt: '2', controlAttempt: '2' });
  try { validate(value.evidence, current); } finally { value.cleanup(); }
});

test('accepts authenticated same-run evidence retained from an earlier attempt', () => {
  const value = fixture();
  try { validate(value.evidence, current); } finally { value.cleanup(); }
});

for (const [name, field, replacement] of [
  ['source revision', 'source_revision', 'b'.repeat(40)],
  ['run ID', 'run_id', '54321'],
  ['target', 'target', 'aarch64-unknown-linux-gnu'],
  ['runner architecture', 'runner_arch', 'ARM64'],
  ['partition name', 'partition', 'runtime'],
  ['future attempt', 'run_attempt', '3'],
  ['zero attempt', 'run_attempt', '0'],
]) {
  test(`rejects mismatched ${name}`, () => {
    const value = fixture({ mutate: (identity, partition) => {
      if (partition === 'core') identity[field] = replacement;
    } });
    try { assert.throws(() => validate(value.evidence, current)); } finally { value.cleanup(); }
  });
}

test('rejects a missing partition identity', () => {
  const value = fixture();
  try {
    fs.unlinkSync(path.join(value.evidence, 'partitions', 'runtime', 'identity.json'));
    assert.throws(() => validate(value.evidence, current));
  } finally { value.cleanup(); }
});

test('rejects a missing partition and an unexpected duplicate partition entry', () => {
  const missing = fixture();
  try {
    fs.rmSync(path.join(missing.evidence, 'partitions', 'runtime'), { recursive: true });
    assert.throws(() => validate(missing.evidence, current));
  } finally { missing.cleanup(); }

  const duplicate = fixture();
  try {
    fs.mkdirSync(path.join(duplicate.evidence, 'partitions', 'runtime-copy'));
    assert.throws(() => validate(duplicate.evidence, current));
  } finally { duplicate.cleanup(); }
});

test('rejects a missing semantic-control receipt', () => {
  const value = fixture();
  try {
    fs.rmSync(path.join(value.evidence, 'A10-linux-control'), { recursive: true });
    assert.throws(() => validate(value.evidence, current));
  } finally { value.cleanup(); }
});

test('rejects duplicate semantic-control receipts', () => {
  const value = fixture({ extraControls: ['2'] });
  try { assert.throws(() => validate(value.evidence, current)); } finally { value.cleanup(); }
});

test('rejects a semantic-control receipt from another run', () => {
  const value = fixture();
  try {
    const controls = path.join(value.evidence, 'A10-linux-control');
    const original = path.join(controls, '12345-1-X64.AbCd1234');
    const other = path.join(controls, '99999-1-X64.AbCd1234');
    fs.renameSync(original, other);
    assert.throws(() => validate(value.evidence, current));
  } finally { value.cleanup(); }
});

test('rejects altered semantic-control identity and result', () => {
  const value = fixture();
  try {
    const dir = path.join(value.evidence, 'A10-linux-control', '12345-1-X64.AbCd1234');
    fs.writeFileSync(path.join(dir, 'identity.txt'), 'head=wrong\n');
    assert.throws(() => validate(value.evidence, current));
    fs.writeFileSync(path.join(dir, 'identity.txt'),
      `head=${current.GITHUB_SHA}\nrunner_arch=X64\nrun_id=12345\nrun_attempt=1\n`);
    fs.writeFileSync(path.join(dir, 'control-result.txt'), 'EXPECTED_SEMANTIC_FAILURE_CONFIRMED\nextra\n');
    assert.throws(() => validate(value.evidence, current));
  } finally { value.cleanup(); }
});
