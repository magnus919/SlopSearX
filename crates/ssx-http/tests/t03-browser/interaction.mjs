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
      const finish = (value) => {
        clearTimeout(timer);
        cancelAnimationFrame(frame);
        window.removeEventListener(expected.event, onInput, true);
        resolve(value);
      };
      const timer = setTimeout(() => finish({ error: 'input/state/frame deadline exceeded' }), timeoutMs);
      const onInput = (event) => {
        if (!event.isTrusted || (expected.key && event.key !== expected.key)) return;
        if (expected.target && !event.target.closest(expected.target)) return;
        window.removeEventListener(expected.event, onInput, true);
        const start = event.timeStamp;
        if (!Number.isFinite(start) || start < 0 || start > performance.now()) {
          finish({ error: 'invalid browser input timestamp' });
          return;
        }
        const observe = () => {
          if (!matches()) { frame = requestAnimationFrame(observe); return; }
          frame = requestAnimationFrame(() => {
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

export async function timingControls(browser) {
  const page = await browser.newPage();
  const expected = { event: 'click', target: '#respond', selector: '#respond', kind: 'attribute', attribute: 'data-ready', value: '1' };
  try {
    const setup = async (delay) => {
      await page.setContent('<button id="respond">Respond</button>');
      await page.evaluate((delay) => document.querySelector('#respond').addEventListener('click', (event) => {
        const start = performance.now();
        while (performance.now() - start < delay) { /* deliberate slow-handler control */ }
        event.currentTarget.setAttribute('data-ready', '1');
      }), delay);
    };
    await setup(0);
    const fast = await measureInteraction(page, () => page.locator('#respond').click(), expected);
    assert(fast.duration_ms <= 100, `fast timing control exceeded budget: ${fast.duration_ms}`);
    await setup(150);
    const slow = await measureInteraction(page, () => page.locator('#respond').click(), expected);
    assert(slow.duration_ms > 100, 'timing control failed to detect slow handler');
    await setup(0);
    await assert.rejects(measureInteraction(page, async () => {}, expected, 150), /deadline/);
    await page.setContent('<button id="respond">No state change</button>');
    await assert.rejects(measureInteraction(page, () => page.locator('#respond').click(), expected, 250), /deadline/);
    await setup(0);
    await assert.rejects(measureInteraction(page, () => page.locator('#respond').evaluate((element) => element.click()), expected, 150), /deadline/);
    return { fast_ms: fast.duration_ms, slow_ms: slow.duration_ms, absent_input_rejected: true, absent_state_rejected: true, untrusted_input_rejected: true };
  } finally {
    await page.close();
  }
}
