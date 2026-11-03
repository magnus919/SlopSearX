import fs from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

export const REQUIRED_PARTITIONS = Object.freeze(['prerequisites', 'core', 'runtime', 'adapters']);
const EXPECTED_CONTROL_RESULT = 'EXPECTED_SEMANTIC_FAILURE_CONFIRMED\n';

function requireValue(condition, message) {
  if (!condition) throw new Error(message);
}

function attemptNumber(value, label) {
  requireValue(typeof value === 'string' && /^[1-9][0-9]*$/.test(value), `${label} must be a positive decimal attempt`);
  const valueNumber = Number(value);
  requireValue(Number.isSafeInteger(valueNumber), `${label} must be a safe integer`);
  return valueNumber;
}

export function validatePartitionIdentities(identities, expected) {
  requireValue(Array.isArray(identities), 'partition identities must be an array');
  const seen = new Set();
  for (const identity of identities) {
    requireValue(identity && typeof identity === 'object' && !Array.isArray(identity), 'partition identity must be an object');
    requireValue(REQUIRED_PARTITIONS.includes(identity.partition), 'unexpected partition identity');
    requireValue(!seen.has(identity.partition), `duplicate partition identity: ${identity.partition}`);
    seen.add(identity.partition);
    requireValue(identity.source_revision === expected.sourceRevision, 'partition source revision mismatch');
    requireValue(identity.target === expected.target, 'partition target mismatch');
    requireValue(identity.run_id === expected.runId, 'partition workflow run mismatch');
    requireValue(identity.runner_arch === expected.runnerArch, 'partition runner architecture mismatch');
    const attempt = attemptNumber(identity.run_attempt, 'partition run_attempt');
    requireValue(attempt <= expected.runAttempt, 'partition evidence comes from a future workflow attempt');
  }
  for (const partition of REQUIRED_PARTITIONS) requireValue(seen.has(partition), `missing partition identity: ${partition}`);
  return new Map(identities.map((identity) => [identity.partition, attemptNumber(identity.run_attempt, 'partition run_attempt')]));
}

export function validateControlEvidence(entries, expected, prerequisiteAttempt) {
  requireValue(Array.isArray(entries) && entries.length === 1, 'expected exactly one semantic-control receipt');
  const entry = entries[0];
  requireValue(entry && typeof entry.directoryName === 'string', 'semantic-control directory name missing');
  const escapedRun = expected.runId.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  const escapedArch = expected.runnerArch.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  const match = entry.directoryName.match(new RegExp(`^${escapedRun}-([1-9][0-9]*)-${escapedArch}\\.([A-Za-z0-9]+)$`));
  requireValue(match, 'semantic-control directory identity mismatch');
  const attempt = attemptNumber(match[1], 'semantic-control run_attempt');
  requireValue(attempt <= expected.runAttempt, 'semantic-control evidence comes from a future workflow attempt');
  const identity = entry.identity;
  requireValue(identity && typeof identity === 'object' && !Array.isArray(identity), 'semantic-control identity missing');
  requireValue(identity.source_revision === expected.sourceRevision, 'semantic-control source revision mismatch');
  requireValue(identity.target === expected.target, 'semantic-control target mismatch');
  requireValue(identity.run_id === expected.runId, 'semantic-control workflow run mismatch');
  requireValue(identity.runner_arch === expected.runnerArch, 'semantic-control runner architecture mismatch');
  requireValue(attemptNumber(identity.run_attempt, 'semantic-control identity run_attempt') === attempt, 'semantic-control path and identity attempt mismatch');
  requireValue(attempt === prerequisiteAttempt, 'semantic-control attempt differs from its prerequisites partition');
  requireValue(entry.regularFiles === true, 'semantic-control receipt files must be regular files');
  requireValue(entry.result === EXPECTED_CONTROL_RESULT, 'semantic-control result missing, duplicated, or altered');
  return attempt;
}

function readRegularFile(file) {
  const info = fs.lstatSync(file);
  requireValue(info.isFile() && !info.isSymbolicLink(), `expected a regular file: ${file}`);
  return fs.readFileSync(file, 'utf8');
}

export function validateFoundationEvidence(evidenceRoot, expected) {
  const identities = REQUIRED_PARTITIONS.map((partition) => {
    const identityPath = path.join(evidenceRoot, 'partitions', partition, 'identity.json');
    return JSON.parse(readRegularFile(identityPath));
  });
  const attempts = validatePartitionIdentities(identities, expected);
  const controlRoot = path.join(evidenceRoot, 'A10-linux-control');
  const entries = fs.readdirSync(controlRoot, { withFileTypes: true }).map((directory) => {
    requireValue(directory.isDirectory() && !directory.isSymbolicLink(), 'unexpected semantic-control artifact entry');
    const directoryPath = path.join(controlRoot, directory.name);
    const receiptFiles = fs.readdirSync(directoryPath).sort();
    requireValue(receiptFiles.length === 2 && receiptFiles[0] === 'control-identity.json' && receiptFiles[1] === 'control-result.txt', 'semantic-control receipt has unexpected or missing files');
    const identity = JSON.parse(readRegularFile(path.join(directoryPath, 'control-identity.json')));
    const result = readRegularFile(path.join(directoryPath, 'control-result.txt'));
    return { directoryName: directory.name, identity, result, regularFiles: true };
  });
  validateControlEvidence(entries, expected, attempts.get('prerequisites'));
  return attempts;
}

function main(argv) {
  const [evidenceRoot, sourceRevision, target, runId, runAttempt, runnerArch] = argv;
  requireValue(argv.length === 6, 'usage: node scripts/foundation-aggregation.mjs EVIDENCE_ROOT SOURCE_SHA TARGET RUN_ID RUN_ATTEMPT RUNNER_ARCH');
  const expected = { sourceRevision, target, runId, runAttempt: attemptNumber(runAttempt, 'GITHUB_RUN_ATTEMPT'), runnerArch };
  const attempts = validateFoundationEvidence(evidenceRoot, expected);
  console.log(`Authenticated Foundation partitions for run ${runId}: ${[...attempts].map(([name, attempt]) => `${name}=attempt-${attempt}`).join(', ')}`);
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  try {
    main(process.argv.slice(2));
  } catch (error) {
    console.error(`Foundation evidence rejected: ${error.message}`);
    process.exitCode = 1;
  }
}
