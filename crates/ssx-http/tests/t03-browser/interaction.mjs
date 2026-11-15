import assert from 'node:assert/strict';

export const INTERACTION_DEFINITION = 'trusted-event-timestamp-to-state-and-next-frame-v1';

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

async function prepareTrustedClick(page, selector, timeoutMs = 5000) {
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
        const nextFrame = () => new Promise((resolveFrame) => {
          const remaining = deadline - performance.now();
          if (remaining <= 0) {
            resolveFrame(false);
            return;
          }
          let settled = false;
          let timer;
          const finish = (advanced) => {
            if (settled) return;
            settled = true;
            clearTimeout(timer);
            resolveFrame(advanced);
          };
          timer = setTimeout(() => finish(false), remaining);
          requestAnimationFrame(() => {
            framesObserved += 1;
            finish(true);
          });
        });
        for (let index = 0; index < 2; index += 1) {
          if (!await nextFrame()) break;
        }
        const final = inspect();
        const rectStable = sameBounds(initial.bounds, final.bounds);
        const visibility = document.visibilityState;
        return {
          frames_observed: framesObserved,
          visibility,
          initial,
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

  const ready = state !== null
    && bringToFrontError === null
    && trialClickError === null
    && state.frames_observed === 2
    && state.visibility === 'visible'
    && state.initial?.connected === true
    && state.initial?.visible === true
    && state.initial?.enabled === true
    && state.initial?.center_hit_target === true
    && state.final?.connected === true
    && state.final?.visible === true
    && state.final?.enabled === true
    && state.final?.center_hit_target === true
    && state.rect_stable === true;

  return {
    ready,
    frames_observed: state?.frames_observed ?? 0,
    visibility: state?.visibility ?? null,
    initial: state?.initial ?? null,
    final: state?.final ?? null,
    rect_stable: state?.rect_stable ?? false,
    browser_ready_ms: state?.browser_ready_ms ?? null,
    bring_to_front_ms: bringToFrontMs,
    bring_to_front_error: bringToFrontError,
    trial_click_ms: trialClickMs,
    trial_click_passed: trialClickError === null,
    trial_click_error: trialClickError,
    elapsed_ms: performance.now() - started,
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
    const trustedClick = async (name, setupMs) => {
      const phase = {
        control: name,
        setup_ms: setupMs,
        readiness: null,
        action_started: false,
        action_completed: false,
        action_driver_ms: null,
        interaction_ms: null,
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
        phase.interaction_ms = sample.duration_ms;
        return sample;
      } catch (error) {
        phase.action_completed = actionCompleted;
        if (error?.readiness) phase.readiness = error.readiness;
        phase.error = error instanceof Error ? error.message : String(error);
        throw error;
      }
    };
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
