import assert from 'node:assert/strict';

export const INTERACTION_DEFINITION = 'trusted-event-timestamp-to-state-and-next-frame-v1';

const READINESS_OBSERVATION_GRACE_MS = 1000;
const TRANSIENT_RESTORE_TIMEOUT_MS = 1000;

function readinessAcceptanceBudgetMs(timeoutMs, includesSetup) {
  return timeoutMs * (includesSetup ? 4 : 3) + READINESS_OBSERVATION_GRACE_MS;
}

// Two frame callbacks give the updated state a rendering opportunity. This is
// a browser-frame response proxy, not Event Timing/INP or proof of physical paint.
export async function measureInteraction(page, action, expected, timeoutMs = 1500) {
  await page.evaluate(({ expected, timeoutMs }) => {
    const matches = () => {
      const element = document.querySelector(expected.selector);
      if (!element) return false;
      if (expected.kind === 'focus') return document.activeElement === element;
      return element.getAttribute(expected.attribute) === expected.value;
    };
    if (matches()) throw new Error('interaction state already satisfied before input');
    let cleanup;
    const result = new Promise((resolve) => {
      let frame;
      let inputSeen = false;
      let framesObserved = 0;
      const armedAt = performance.now();
      const finish = (value) => {
        clearTimeout(timer);
        cancelAnimationFrame(frame);
        window.removeEventListener(expected.event, onInput, true);
        resolve(value);
      };
      const timer = setTimeout(() => finish({ error: `input/state/frame deadline exceeded: ${JSON.stringify({ input_seen: inputSeen, state_matches: matches(), frames_observed: framesObserved, visibility: document.visibilityState, elapsed_ms: performance.now() - armedAt })}` }), timeoutMs);
      const onInput = (event) => {
        if (!event.isTrusted || (expected.key && event.key !== expected.key)) return;
        if (expected.target && !event.target.closest(expected.target)) return;
        window.removeEventListener(expected.event, onInput, true);
        inputSeen = true;
        const start = event.timeStamp;
        if (!Number.isFinite(start) || start < 0 || start > performance.now()) {
          finish({ error: 'invalid browser input timestamp' });
          return;
        }
        const observe = () => {
          framesObserved += 1;
          if (!matches()) { frame = requestAnimationFrame(observe); return; }
          frame = requestAnimationFrame(() => {
            framesObserved += 1;
            if (!matches()) { frame = requestAnimationFrame(observe); return; }
            finish({ event: event.type, trusted: event.isTrusted, state_observed: true, duration_ms: performance.now() - start });
          });
        };
        frame = requestAnimationFrame(observe);
      };
      window.addEventListener(expected.event, onInput, true);
      cleanup = () => finish({ error: 'interaction cancelled' });
    });
    window.__t03Interaction = { result, cleanup };
  }, { expected, timeoutMs });
  const driverStart = performance.now();
  try {
    await action();
    const sample = await page.evaluate(() => window.__t03Interaction.result);
    assert(!sample.error, sample.error);
    assert(Number.isFinite(sample.duration_ms) && sample.duration_ms >= 0, 'invalid interaction duration');
    return { ...sample, driver_ms: performance.now() - driverStart };
  } finally {
    await page.evaluate(() => { window.__t03Interaction?.cleanup(); delete window.__t03Interaction; });
  }
}

async function prepareTrustedClick(page, selector, timeoutMs = 5000, beforeObserve = null) {
  const started = performance.now();
  const bounded = async (promise, label) => {
    let timer;
    try {
      await Promise.race([
        promise,
        new Promise((_, reject) => {
          timer = setTimeout(() => reject(new Error(label + ' exceeded ' + timeoutMs + ' ms')), timeoutMs);
        }),
      ]);
      return null;
    } catch (error) {
      return error instanceof Error ? error.message : String(error);
    } finally {
      clearTimeout(timer);
    }
  };

  const bringStarted = performance.now();
  const bringToFrontError = await bounded(page.bringToFront(), 'bring page to front');
  const bringToFrontMs = performance.now() - bringStarted;

  const trialStarted = performance.now();
  const trialClickError = await bounded(
    page.locator(selector).click({ trial: true, timeout: timeoutMs }),
    'trial click actionability',
  );
  const trialClickMs = performance.now() - trialStarted;

  let state = null;
  let stateError = null;
  let watchdog;
  try {
    if (beforeObserve) {
      const setupError = await bounded(beforeObserve(), 'readiness test setup');
      if (setupError !== null) throw new Error(setupError);
    }
    state = await Promise.race([
      page.evaluate(async ({ selector, timeoutMs }) => {
        const inspect = () => {
          const element = document.querySelector(selector);
          if (!element) {
            return {
              connected: false,
              visible: false,
              enabled: false,
              center_hit_target: false,
              bounds: null,
            };
          }
          const rect = element.getBoundingClientRect();
          const style = getComputedStyle(element);
          const visible = style.display !== 'none'
            && style.visibility !== 'hidden'
            && rect.width > 0
            && rect.height > 0
            && element.getClientRects().length > 0;
          const hit = document.elementFromPoint(rect.left + rect.width / 2, rect.top + rect.height / 2);
          return {
            connected: element.isConnected,
            visible,
            enabled: !element.matches(':disabled'),
            center_hit_target: Boolean(hit && (hit === element || element.contains(hit))),
            bounds: { x: rect.x, y: rect.y, width: rect.width, height: rect.height },
          };
        };
        const sameBounds = (left, right) => Boolean(left && right
          && ['x', 'y', 'width', 'height'].every((key) => Math.abs(left[key] - right[key]) <= 0.5));
        const startedAt = performance.now();
        const deadline = startedAt + timeoutMs;
        const initial = inspect();
        let framesObserved = 0;
        const frameSnapshots = [];
        const nextFrame = () => new Promise((resolveFrame) => {
          const remaining = deadline - performance.now();
          if (remaining <= 0) {
            resolveFrame(false);
            return;
          }
          let settled = false;
          let timer;
          let animationFrameId;
          const finish = (advanced) => {
            if (settled) return;
            settled = true;
            clearTimeout(timer);
            if (!advanced && animationFrameId !== undefined) {
              cancelAnimationFrame(animationFrameId);
            }
            resolveFrame(advanced);
          };
          timer = setTimeout(() => finish(false), remaining);
          animationFrameId = requestAnimationFrame(() => {
            if (settled) return;
            frameSnapshots.push(inspect());
            framesObserved += 1;
            finish(true);
          });
        });
        for (let index = 0; index < 2; index += 1) {
          if (!await nextFrame()) break;
        }
        const final = inspect();
        const rectStable = [initial, ...frameSnapshots, final]
          .every((snapshot) => sameBounds(initial.bounds, snapshot.bounds));
        const visibility = document.visibilityState;
        return {
          frames_observed: framesObserved,
          visibility,
          initial,
          frame_snapshots: frameSnapshots,
          final,
          rect_stable: rectStable,
          browser_ready_ms: performance.now() - startedAt,
        };
      }, { selector, timeoutMs }),
      new Promise((_, reject) => {
        watchdog = setTimeout(
          () => reject(new Error('browser readiness observation exceeded ' + (timeoutMs + 1000) + ' ms')),
          timeoutMs + 1000,
        );
      }),
    ]);
  } catch (error) {
    stateError = error instanceof Error ? error.message : String(error);
  } finally {
    clearTimeout(watchdog);
  }

  const observedSnapshots = state === null
    ? []
    : [
      state.initial,
      ...(Array.isArray(state.frame_snapshots) ? state.frame_snapshots : []),
      state.final,
    ];
  const acceptanceBudgetMs = readinessAcceptanceBudgetMs(timeoutMs, beforeObserve !== null);
  const elapsedMs = performance.now() - started;
  const snapshotActionable = (snapshot) => snapshot?.connected === true
    && snapshot?.visible === true
    && snapshot?.enabled === true
    && snapshot?.center_hit_target === true;
  const ready = state !== null
    && bringToFrontError === null
    && trialClickError === null
    && state.frames_observed === 2
    && state.frame_snapshots?.length === 2
    && state.visibility === 'visible'
    && observedSnapshots.length === 4
    && observedSnapshots.every(snapshotActionable)
    && state.rect_stable === true
    && elapsedMs <= acceptanceBudgetMs;

  return {
    ready,
    frames_observed: state?.frames_observed ?? 0,
    visibility: state?.visibility ?? null,
    initial: state?.initial ?? null,
    frame_snapshots: state?.frame_snapshots ?? [],
    final: state?.final ?? null,
    rect_stable: state?.rect_stable ?? false,
    browser_ready_ms: state?.browser_ready_ms ?? null,
    bring_to_front_ms: bringToFrontMs,
    bring_to_front_error: bringToFrontError,
    trial_click_ms: trialClickMs,
    trial_click_passed: trialClickError === null,
    trial_click_error: trialClickError,
    elapsed_ms: elapsedMs,
    acceptance_budget_ms: acceptanceBudgetMs,
    timeout_ms: timeoutMs,
    error: stateError,
  };
}

export async function timingControls(browser) {
  const page = await browser.newPage();
  let control = 'setup';
  const expected = { event: 'click', target: '#respond', selector: '#respond', kind: 'attribute', attribute: 'data-ready', value: '1' };
  const controls = {
    fast_ms: null,
    slow_ms: null,
    transient_bounds_rejected: false,
    transient_bounds_evidence: null,
    transient_bounds_restore_error: null,
    absent_input_rejected: false,
    absent_state_rejected: false,
    untrusted_input_rejected: false,
    pre_input: [],
    cleanup_error: null,
  };
  let primaryError = null;
  try {
    const setup = async (delay) => {
      await page.setContent('<button id="respond">Respond</button>');
      await page.evaluate((delay) => document.querySelector('#respond').addEventListener('click', (event) => {
        const start = performance.now();
        while (performance.now() - start < delay) { /* deliberate slow-handler control */ }
        event.currentTarget.setAttribute('data-ready', '1');
      }), delay);
    };
    const rejectTransientMovement = async () => {
      await page.setContent('<button id="respond">Respond</button>');
      let readiness = null;
      let preparationError = null;
      try {
        readiness = await prepareTrustedClick(page, '#respond', 5000, async () => {
          await page.evaluate(() => {
            const original = window.requestAnimationFrame.bind(window);
            let frame = 0;
            window.__ssxT03OriginalRequestAnimationFrame = original;
            window.requestAnimationFrame = (callback) => original((timestamp) => {
              frame += 1;
              const target = document.querySelector('#respond');
              if (frame === 1) target.style.transform = 'translateX(1px)';
              if (frame === 2) target.style.transform = '';
              callback(timestamp);
            });
          });
        });
      } catch (error) {
        preparationError = error;
      }
      // Retain observations before attempting best-effort page restoration. A
      // failed restore must not erase the evidence that explains the failure.
      controls.transient_bounds_evidence = readiness;

      let restoreTimer;
      let restoreError = null;
      try {
        await Promise.race([
          Promise.resolve().then(() => page.evaluate(() => {
            const original = window.__ssxT03OriginalRequestAnimationFrame;
            if (typeof original === 'function') window.requestAnimationFrame = original;
            delete window.__ssxT03OriginalRequestAnimationFrame;
            document.querySelector('#respond')?.style.removeProperty('transform');
          })),
          new Promise((_, reject) => {
            restoreTimer = setTimeout(
              () => reject(new Error(`transient bounds restore exceeded ${TRANSIENT_RESTORE_TIMEOUT_MS} ms`)),
              TRANSIENT_RESTORE_TIMEOUT_MS,
            );
          }),
        ]);
      } catch (error) {
        restoreError = error instanceof Error ? error.message : String(error);
      } finally {
        clearTimeout(restoreTimer);
      }
      controls.transient_bounds_restore_error = restoreError;
      if (preparationError) {
        if (restoreError !== null && typeof preparationError === 'object') {
          preparationError.transient_bounds_restore_error = restoreError;
        }
        throw preparationError;
      }
      if (restoreError !== null) {
        throw new Error(`transient bounds restore failed: ${restoreError}`);
      }
      const initialX = readiness.initial?.bounds?.x;
      const firstFrameX = readiness.frame_snapshots?.[0]?.bounds?.x;
      const secondFrameX = readiness.frame_snapshots?.[1]?.bounds?.x;
      const snapshots = [
        readiness.initial,
        ...(Array.isArray(readiness.frame_snapshots) ? readiness.frame_snapshots : []),
        readiness.final,
      ];
      const snapshotActionable = (snapshot) => snapshot?.connected === true
        && snapshot?.visible === true
        && snapshot?.enabled === true
        && snapshot?.center_hit_target === true;
      const rejectedForTransientMovement = readiness.ready === false
        && readiness.frames_observed === 2
        && readiness.visibility === 'visible'
        && readiness.bring_to_front_error === null
        && readiness.trial_click_passed === true
        && readiness.trial_click_error === null
        && readiness.elapsed_ms <= readiness.acceptance_budget_ms
        && readiness.error === null
        && readiness.rect_stable === false
        && snapshots.length === 4
        && snapshots.every(snapshotActionable)
        && Number.isFinite(initialX)
        && Number.isFinite(firstFrameX)
        && Number.isFinite(secondFrameX)
        && Math.abs(firstFrameX - initialX) > 0.5
        && Math.abs(secondFrameX - initialX) <= 0.5;
      return { rejected: rejectedForTransientMovement, evidence: readiness };
    };
    const trustedClick = async (name, setupMs) => {
      const phase = {
        control: name,
        setup_ms: setupMs,
        readiness: null,
        action_started: false,
        action_completed: false,
        action_driver_ms: null,
        error: null,
      };
      controls.pre_input.push(phase);
      let actionStartedAt = null;
      let actionCompleted = false;
      try {
        phase.readiness = await prepareTrustedClick(page, '#respond');
        assert(phase.readiness.ready, 'pre-input readiness failed: ' + JSON.stringify(phase.readiness));
        const sample = await measureInteraction(page, async () => {
          phase.action_started = true;
          actionStartedAt = performance.now();
          try {
            await page.locator('#respond').click({ timeout: 1500 });
            actionCompleted = true;
          } finally {
            phase.action_driver_ms = performance.now() - actionStartedAt;
          }
        }, expected);
        phase.action_completed = actionCompleted;
        return sample;
      } catch (error) {
        phase.action_completed = actionCompleted;
        if (error?.readiness) phase.readiness = error.readiness;
        phase.error = error instanceof Error ? error.message : String(error);
        throw error;
      }
    };
    control = 'transient bounds movement';
    const transientBounds = await rejectTransientMovement();
    controls.transient_bounds_rejected = transientBounds.rejected;
    controls.transient_bounds_evidence = transientBounds.evidence;
    assert(controls.transient_bounds_rejected, 'transient bounds movement was not rejected');
    let setupStarted = performance.now();
    await setup(0);
    control = 'fast trusted click';
    const fast = await trustedClick(control, performance.now() - setupStarted);
    controls.fast_ms = fast.duration_ms;
    assert(fast.duration_ms <= 100, `fast timing control exceeded budget: ${fast.duration_ms}`);
    setupStarted = performance.now();
    await setup(150);
    control = 'slow trusted click';
    const slow = await trustedClick(control, performance.now() - setupStarted);
    controls.slow_ms = slow.duration_ms;
    assert(slow.duration_ms > 100, 'timing control failed to detect slow handler');
    await setup(0);
    control = 'absent input';
    await assert.rejects(measureInteraction(page, async () => {}, expected, 150), /deadline/);
    controls.absent_input_rejected = true;
    await page.setContent('<button id="respond">No state change</button>');
    control = 'absent state';
    await assert.rejects(measureInteraction(page, () => page.locator('#respond').click(), expected, 250), /deadline/);
    controls.absent_state_rejected = true;
    await setup(0);
    control = 'untrusted input';
    await assert.rejects(measureInteraction(page, () => page.locator('#respond').evaluate((element) => element.click()), expected, 150), /deadline/);
    controls.untrusted_input_rejected = true;
    return controls;
  } catch (error) {
    if (error && typeof error === 'object') error.timingControlsEvidence = controls;
    primaryError = new Error(`timing control ${control}: ${error.message}`, { cause: error });
    primaryError.timingControlsEvidence = controls;
    throw primaryError;
  } finally {
    try {
      await page.close();
    } catch (closeError) {
      controls.cleanup_error = closeError instanceof Error ? closeError.message : String(closeError);
      if (primaryError) {
        primaryError.cleanup_error = controls.cleanup_error;
      } else {
        const wrapped = new Error(`timing control browser close failed: ${controls.cleanup_error}`, { cause: closeError });
        wrapped.timingControlsEvidence = controls;
        throw wrapped;
      }
    }
  }
}
