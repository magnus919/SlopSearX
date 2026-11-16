import assert from 'node:assert/strict';
import test from 'node:test';
import { timingControls } from './interaction.mjs';

function transientReadiness() {
  const snapshot = (x) => ({
    connected: true,
    visible: true,
    enabled: true,
    center_hit_target: true,
    bounds: { x, y: 20, width: 80, height: 30 },
  });
  return {
    ready: false,
    frames_observed: 2,
    visibility: 'visible',
    initial: snapshot(10),
    frame_snapshots: [snapshot(11), snapshot(10)],
    final: snapshot(10),
    rect_stable: false,
    browser_ready_ms: 5,
    bring_to_front_ms: 1,
    bring_to_front_error: null,
    trial_click_ms: 2,
    trial_click_passed: true,
    trial_click_error: null,
    elapsed_ms: 8,
    acceptance_budget_ms: 21000,
    timeout_ms: 5000,
    error: null,
  };
}

function fakeBrowser(restore, close = async () => {}) {
  const readiness = transientReadiness();
  let evaluation = 0;
  const page = {
    async setContent() {},
    async bringToFront() {},
    locator() { return { async click() {} }; },
    async evaluate() {
      evaluation += 1;
      if (evaluation === 2) return readiness;
      if (evaluation === 3) return restore();
    },
    close,
  };
  return { browser: { async newPage() { return page; } }, readiness, evaluations: () => evaluation };
}

test('transient restoration failure keeps readiness snapshots and page-close diagnostics', async () => {
  const fake = fakeBrowser(
    async () => { throw new Error('injected restore failure'); },
    async () => { throw new Error('injected page-close failure'); },
  );

  await assert.rejects(timingControls(fake.browser), (error) => {
    assert.match(error.message, /timing control transient bounds movement/);
    const evidence = error.timingControlsEvidence;
    assert.ok(evidence);
    assert.equal(evidence.transient_bounds_evidence.ready, false);
    assert.deepEqual(evidence.transient_bounds_evidence.initial.bounds, fake.readiness.initial.bounds);
    assert.deepEqual(evidence.transient_bounds_evidence.frame_snapshots, fake.readiness.frame_snapshots);
    assert.deepEqual(evidence.transient_bounds_evidence.final.bounds, fake.readiness.final.bounds);
    assert.equal(evidence.transient_bounds_restore_error, 'injected restore failure');
    assert.equal(evidence.cleanup_error, 'injected page-close failure');
    assert.equal(error.cleanup_error, 'injected page-close failure');
    return true;
  });
  assert.equal(fake.evaluations(), 3);
});

test('transient restoration has a bounded deadline', async () => {
  const fake = fakeBrowser(() => new Promise(() => {}));

  await assert.rejects(timingControls(fake.browser), (error) => {
    assert.match(error.message, /transient bounds restore exceeded 1000 ms/);
    const evidence = error.timingControlsEvidence;
    assert.ok(evidence);
    assert.equal(evidence.transient_bounds_evidence.ready, false);
    assert.equal(evidence.transient_bounds_evidence.initial.bounds.x, 10);
    assert.match(evidence.transient_bounds_restore_error, /exceeded 1000 ms/);
    assert.equal(evidence.cleanup_error, null);
    return true;
  });
  assert.equal(fake.evaluations(), 3);
});
