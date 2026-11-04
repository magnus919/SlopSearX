import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFileSync, writeFileSync, lstatSync, existsSync } from 'node:fs';
import { dirname, isAbsolute, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { gzipSync } from 'node:zlib';
import { chromium } from 'playwright-core';

const here = dirname(fileURLToPath(import.meta.url));
const sourcePath = fileURLToPath(import.meta.url);
const packagePath = join(here, 'package.json');
const supportedScenarios = new Set([
  'contract',
  'cached-error',
  'rate-limited',
  'html-disabled',
  'unavailable',
]);
const requiredByScenario = {
  contract: [
    'results-normal',
    'optional-metadata',
    'empty-suggestions',
    'partial-results',
    'all-source-failure',
    'filter-rejections',
    'malicious-content',
    'page-two-history',
    'dark-darker-storage',
    'progressive-explanations',
    'keyboard-only',
    'viewport-desktop',
    'viewport-narrow',
    'viewport-320css',
    'reduced-motion',
  ],
  'cached-error': ['cached-error'],
  'rate-limited': ['rate-limited'],
  'html-disabled': ['disabled-format'],
  unavailable: ['unavailable-source'],
};
const viewportHistory = [];
const completed = [];
const failures = [];
const unexpectedRequests = [];
const consoleErrors = [];
const httpErrorResponses = [];
const httpErrorConsoleStatuses = [];
const pageErrors = [];
const requestFailures = [];
const artifacts = [];
const interactionSamples = [];
const handoffSamples = [];
let browser = null;
let browserClosed = false;
let browserInfo = { name: 'chromium', version: '', playwright_version: '' };
let budgets = {};
let evidenceDirectory = '';
let baseUrl = '';
let scenario = '';

function sha256(bytes) {
  return createHash('sha256').update(bytes).digest('hex');
}

function addFailure(id, error) {
  const message = error instanceof Error ? error.message : String(error);
  failures.push(`${id}: ${message}`);
}

async function check(id, body) {
  try {
    await body();
    completed.push(id);
  } catch (error) {
    addFailure(id, error);
  }
}

function pathFor(name) {
  const path = join(evidenceDirectory, name);
  assert(isAbsolute(path));
  assert(!existsSync(path), `refusing to overwrite existing evidence: ${path}`);
  return path;
}

function attachPageEvidence(page) {
  if (page.__t03EvidenceAttached) return;
  page.__t03EvidenceAttached = true;
  page.on('console', (message) => {
    if (message.type() !== 'error') return;
    const match = /^Failed to load resource: the server responded with a status of (\d{3}) \(/.exec(message.text());
    if (match) httpErrorConsoleStatuses.push(Number(match[1]));
    else consoleErrors.push(message.text());
  });
  page.on('response', (response) => {
    if (response.status() >= 400) httpErrorResponses.push({ status: response.status(), url: response.url() });
  });
  page.on('pageerror', (error) => pageErrors.push(error.message));
  page.on('crash', () => pageErrors.push('Chromium page crashed'));
  page.on('requestfailed', (request) => {
    requestFailures.push(`${request.url()}: ${request.failure()?.errorText ?? 'request failed'}`);
  });
}

async function newContext(options = {}) {
  const context = await browser.newContext({
    viewport: options.viewport ?? { width: 1440, height: 900 },
    reducedMotion: options.reducedMotion ?? 'no-preference',
  });
  if (options.initScript) await context.addInitScript(options.initScript);
  context.on('page', attachPageEvidence);
  await context.route('**/*', async (route) => {
    const requestUrl = route.request().url();
    let requestOrigin = '';
    try {
      requestOrigin = new URL(requestUrl).origin;
    } catch {
      unexpectedRequests.push(requestUrl);
      await route.abort();
      return;
    }
    if (requestOrigin !== new URL(baseUrl).origin) {
      unexpectedRequests.push(requestUrl);
      await route.abort();
      return;
    }
    await route.continue();
  });
  return context;
}

function recordViewport(label, viewport, scrollWidth, dpr) {
  viewportHistory.push({
    label,
    css_width: viewport.width,
    css_height: viewport.height,
    scroll_width: scrollWidth,
    device_pixel_ratio: dpr,
    horizontal_overflow: scrollWidth > viewport.width,
    actual_browser_zoom_200_percent: false,
  });
}

async function viewportObservation(page, label) {
  const viewport = page.viewportSize();
  const measured = await page.evaluate(() => ({
    scrollWidth: document.documentElement.scrollWidth,
    dpr: window.devicePixelRatio,
  }));
  recordViewport(label, viewport, measured.scrollWidth, measured.dpr);
  return measured.scrollWidth <= viewport.width;
}

async function screenshot(page, name, kind = 'screenshot') {
  const path = pathFor(name);
  const viewport = page.viewportSize();
  const bytes = await page.screenshot({ path, fullPage: true, animations: 'disabled' });
  artifacts.push({
    path,
    sha256: sha256(bytes),
    bytes: bytes.byteLength,
    kind,
    width: viewport.width,
    height: viewport.height,
  });
}

async function navigate(page, pathname) {
  const response = await page.goto(new URL(pathname, baseUrl).href, { waitUntil: 'domcontentloaded' });
  assert(response, `navigation did not receive an HTTP response for ${pathname}`);
  return response;
}

async function navigateBy(page, action) {
  const navigation = page.waitForNavigation({ waitUntil: 'domcontentloaded' });
  await action();
  const response = await navigation;
  assert(response, 'form/link activation did not produce a navigation response');
  const handoff = await page.evaluate(() => {
    const entry = performance.getEntriesByType('navigation')[0];
    if (!entry || !Number.isFinite(entry.domContentLoadedEventEnd) || !Number.isFinite(entry.responseEnd)) return null;
    return entry.domContentLoadedEventEnd - entry.responseEnd;
  });
  handoffSamples.push(handoff);
  return response;
}

async function searchByQuery(page, query, extra = {}) {
  const params = new URLSearchParams({ q: query, ...extra });
  return navigate(page, `/search?${params.toString()}`);
}

async function clickAndMeasure(page, locator) {
  const start = performance.now();
  await locator.click();
  await page.evaluate(() => new Promise((resolveFrame) => requestAnimationFrame(resolveFrame)));
  interactionSamples.push(performance.now() - start);
}

async function keyboardAndMeasure(page, key) {
  const start = performance.now();
  await page.keyboard.press(key);
  await page.evaluate(() => new Promise((resolveFrame) => requestAnimationFrame(resolveFrame)));
  interactionSamples.push(performance.now() - start);
}

async function landingAndSearch(page, query = 'valkey', category = 'general') {
  await navigate(page, '/');
  const input = page.getByRole('searchbox', { name: 'Search SlopSearX' });
  const scope = page.locator('form#portal-landing-search select[name="categories"]');
  await input.fill(query);
  await scope.selectOption(category);
  const response = await navigateBy(page, () => input.press('Enter'));
  assert.equal(response.status(), 200);
  const url = new URL(page.url());
  assert.equal(url.searchParams.get('q'), query);
  assert.equal(url.searchParams.get('categories'), category);
  return response;
}

async function pageMetric(page) {
  return page.evaluate(() => {
    const navigation = performance.getEntriesByType('navigation')[0];
    const paint = performance.getEntriesByName('first-contentful-paint')[0];
    return {
      html: document.documentElement.outerHTML,
      style: document.querySelector('style')?.textContent ?? '',
      script: document.querySelector('script')?.textContent ?? '',
      fcp_ms: paint?.startTime ?? null,
      cls: window.__t03Cls ?? null,
      response_end_ms: navigation?.responseEnd ?? null,
      dom_content_loaded_ms: navigation?.domContentLoadedEventEnd ?? null,
    };
  });
}

async function settleLayout(page) {
  await page.evaluate(() => new Promise((resolveFrame) =>
    requestAnimationFrame(() => requestAnimationFrame(resolveFrame)),
  ));
  await page.waitForTimeout(250);
}

function metric(observed, limit) {
  const valid = Number.isFinite(observed);
  return { observed: valid ? observed : null, limit, pass: valid && observed <= limit };
}

function percentile95(values) {
  if (values.length === 0) return null;
  const ordered = [...values].sort((left, right) => left - right);
  return ordered[Math.ceil(0.95 * ordered.length) - 1];
}

async function collectContractBudgets(landingMetric, resultsMetric, page) {
  const htmlBytes = Buffer.byteLength(landingMetric.html, 'utf8');
  const gzipBytes = gzipSync(Buffer.from(landingMetric.html, 'utf8')).byteLength;
  const cssJsBytes = Buffer.byteLength(landingMetric.style, 'utf8') + Buffer.byteLength(landingMetric.script, 'utf8');
  const handoffP95 = percentile95(handoffSamples);
  const interactionP95 = percentile95(interactionSamples);
  budgets = {
    profile: 'synthetic offline T03 fixture; Chromium; Fast 3G emulation (150 ms, 1.6 Mbit/s down, 750 Kbit/s up); 4x CPU throttle; no browser zoom emulation',
    html_uncompressed_bytes: metric(htmlBytes, 51200),
    html_gzip_bytes: metric(gzipBytes, 15360),
    css_js_uncompressed_bytes: metric(cssJsBytes, 30720),
    fcp_ms: metric(landingMetric.fcp_ms, 1500),
    cls: metric(Math.max(landingMetric.cls, resultsMetric.cls), 0.1),
    cls_samples: {
      landing: landingMetric.cls,
      results: resultsMetric.cls,
      result_settle_ms: 250,
    },
    interaction_p95_ms: metric(interactionP95, 100),
    handoff_p95_ms: metric(handoffP95, 150),
  };
  for (const [name, result] of Object.entries(budgets)) {
    if (name === 'profile' || name === 'cls_samples') continue;
    if (!result.pass) addFailure(`budget-${name}`, `observed ${result.observed}, limit ${result.limit}`);
  }
}

async function runContract() {
  const context = await newContext({ viewport: { width: 1440, height: 900 } });
  const page = await context.newPage();
  attachPageEvidence(page);

  const cdp = await context.newCDPSession(page);
  await cdp.send('Network.enable');
  await cdp.send('Network.emulateNetworkConditions', {
    offline: false,
    latency: 150,
    downloadThroughput: (1.6 * 1024 * 1024) / 8,
    uploadThroughput: (750 * 1024) / 8,
    connectionType: 'cellular3g',
  });
  await cdp.send('Emulation.setCPUThrottlingRate', { rate: 4 });
  await context.addInitScript(`(() => {
    window.__t03Cls = 0;
    new PerformanceObserver((list) => {
      for (const entry of list.getEntries()) {
        if (!entry.hadRecentInput) window.__t03Cls += entry.value;
      }
    }).observe({ type: 'layout-shift', buffered: true });
  })();`);

  let landingMetric = null;
  let resultsMetric = null;
  await check('viewport-desktop', async () => {
    await navigate(page, '/');
    await page.waitForFunction(() => performance.getEntriesByName('first-contentful-paint').length > 0, null, { timeout: 2000 });
    assert.equal(await viewportObservation(page, 'desktop-1440x900'), true);
    assert.equal((await page.locator('meta[name="viewport"]').getAttribute('content')), 'width=device-width, initial-scale=1');
    await screenshot(page, 'contract-desktop-landing.png');
    await settleLayout(page);
    landingMetric = await pageMetric(page);
    assert(landingMetric.fcp_ms !== null, 'FCP observation is unavailable');
  });

  await check('results-normal', async () => {
    const response = await landingAndSearch(page, 'valkey', 'general');
    assert.equal(response.status(), 200);
    assert.equal(await page.getByRole('heading', { level: 1 }).innerText(), 'valkey');
    assert(await page.locator('article[data-result-card]').count() > 0);
    assert.equal(await page.locator('details.scope-panel select[name="categories"]').inputValue(), 'general');
    await screenshot(page, 'contract-desktop-results.png');
    await settleLayout(page);
    resultsMetric = await pageMetric(page);
  });

  await check('optional-metadata', async () => {
    const response = await searchByQuery(page, 'climate', {
      engines: 'openalex',
      date_from: '2024-02-29',
      date_to: '2024-03-01',
      language: 'fr',
    });
    assert.equal(response.status(), 200);
    assert.equal(await page.locator('input[name="date_from"][type="date"]').inputValue(), '2024-02-29');
    assert.equal(await page.locator('input[name="date_to"][type="date"]').inputValue(), '2024-03-01');
    assert((await page.locator('a[href*="format=json"]').count()) > 0);
    assert((await page.locator('a[href*="format=yaml"]').count()) > 0);
    const links = await page.locator('a[href*="format=json"],a[href*="format=yaml"]').evaluateAll((els) =>
      els.map((el) => new URL(el.href).origin),
    );
    assert(links.every((origin) => origin === new URL(baseUrl).origin));
  });

  await check('empty-suggestions', async () => {
    const response = await searchByQuery(page, 'empty', { categories: 'general' });
    assert.equal(response.status(), 200);
    assert(await page.getByRole('heading', { name: 'No results found' }).isVisible());
    assert(await page.getByRole('link', { name: 'broader query' }).isVisible());
    assert.equal(await page.locator('body').evaluate((body) => body.querySelectorAll('script').length), 1);
    assert.equal(await page.locator('body').evaluate((body) => body.querySelector('img,svg') === null), true);
  });

  await check('partial-results', async () => {
    const response = await searchByQuery(page, 'partial', { categories: 'general' });
    assert.equal(response.status(), 200);
    assert(await page.getByRole('status').getByText('Some sources could not answer this search.').isVisible());
    assert(await page.locator('article[data-result-card]').count() > 0);
  });

  await check('all-source-failure', async () => {
    const response = await searchByQuery(page, 'all-failed', { categories: 'general' });
    assert.equal(response.status(), 503);
    assert(await page.getByRole('alert').getByText('No configured source returned a result.').isVisible());
    assert.equal(await page.locator('article[data-result-card]').count(), 0);
    await screenshot(page, 'contract-all-source-failure.png');
  });

  await check('filter-rejections', async () => {
    const strict = await searchByQuery(page, '<query>', { safesearch: '2', api_key: 'must-not-render' });
    assert.equal(strict.status(), 400);
    assert(await page.getByRole('alert').isVisible());
    assert.equal(await page.locator('body').innerText().then((text) => text.includes('must-not-render')), false);
    assert.equal(await page.locator('select[name="safesearch"]').getAttribute('aria-invalid'), 'true');
    const sensitive = await searchByQuery(page, 'restricted', { engines: 'hibp' });
    assert.equal(sensitive.status(), 403);
    assert.equal(await page.locator('select[name="engines"]').count(), 0);
    assert.equal(await page.locator('body').getAttribute('data-invalid-field'), 'engines');
  });

  await check('malicious-content', async () => {
    const response = await searchByQuery(page, 'malicious', { categories: 'general' });
    assert.equal(response.status(), 200);
    assert(await page.getByRole('heading', { name: '<img src=x onerror=alert(1)> & source' }).isVisible());
    assert.equal(await page.locator('img,svg').count(), 0);
    assert.equal(await page.locator('a[href^="javascript:"]').count(), 0);
    assert.equal(await page.locator('script').count(), 1);
    await screenshot(page, 'contract-malicious-content.png');
  });

  await check('page-two-history', async () => {
    await landingAndSearch(page, 'valkey', 'general');
    await navigateBy(page, () => page.getByRole('link', { name: 'Try next result page' }).click());
    assert.equal(new URL(page.url()).searchParams.get('pageno'), '2');
    assert.equal(new URL(page.url()).searchParams.get('q'), 'valkey');
    assert.equal(await page.getByRole('heading', { level: 1 }).innerText(), 'valkey');
    await navigateBy(page, () => page.getByRole('link', { name: 'Previous result page' }).click());
    assert.equal(new URL(page.url()).searchParams.get('pageno'), '1');
    const input = page.getByRole('searchbox', { name: 'Search SlopSearX' });
    await input.fill('history');
    await navigateBy(page, () => input.press('Enter'));
    assert.equal(new URL(page.url()).searchParams.get('q'), 'history');
    await page.goBack({ waitUntil: 'domcontentloaded' });
    assert.equal(new URL(page.url()).searchParams.get('q'), 'valkey');
    await page.goForward({ waitUntil: 'domcontentloaded' });
    assert.equal(new URL(page.url()).searchParams.get('q'), 'history');
  });

  await check('dark-darker-storage', async () => {
    await navigate(page, '/');
    assert.equal(await page.locator('html').getAttribute('data-theme'), 'dark');
    await clickAndMeasure(page, page.getByRole('button', { name: 'Use darker mode' }));
    assert.equal(await page.locator('html').getAttribute('data-theme'), 'darker');
    assert.equal(await page.evaluate(() => localStorage.getItem('slopsearx-theme')), 'darker');
    await page.reload({ waitUntil: 'domcontentloaded' });
    assert.equal(await page.locator('html').getAttribute('data-theme'), 'darker');

    const blocked = await newContext({
      viewport: { width: 390, height: 844 },
      initScript: `Object.defineProperty(window, 'localStorage', { configurable: true, get() { throw new Error('blocked by browser policy'); } });`,
    });
    const blockedPage = await blocked.newPage();
    attachPageEvidence(blockedPage);
    await navigate(blockedPage, '/');
    assert.equal(await blockedPage.locator('html').getAttribute('data-theme'), 'dark');
    await clickAndMeasure(blockedPage, blockedPage.getByRole('button', { name: 'Use darker mode' }));
    assert.equal(await blockedPage.locator('html').getAttribute('data-theme'), 'darker');
    await screenshot(blockedPage, 'contract-blocked-storage.png');
    await blocked.close();
  });

  await check('progressive-explanations', async () => {
    await searchByQuery(page, 'valkey', { categories: 'general' });
    const explanation = page.locator('details.result-explanation').first();
    const summary = explanation.locator('summary');
    await summary.focus();
    await keyboardAndMeasure(page, 'Enter');
    assert.equal(await explanation.getAttribute('open') !== null, true);
    assert(await explanation.getByText('Cross-source presence').isVisible());
  });

  await check('keyboard-only', async () => {
    await navigate(page, '/');
    const input = page.getByRole('searchbox', { name: 'Search SlopSearX' });
    assert.equal(await input.evaluate((el) => document.activeElement === el), true);
    await page.keyboard.type('keyboard');
    await page.keyboard.press('Tab');
    assert.equal(await page.evaluate(() => document.activeElement?.classList.contains('search-button')), true);
    await navigateBy(page, () => page.keyboard.press('Enter'));
    assert.equal(new URL(page.url()).searchParams.get('q'), 'keyboard');
    await page.locator('a.brand').focus();
    await keyboardAndMeasure(page, 'j');
    const activationLink = page.locator('.result-link').first();
    assert.equal(await activationLink.evaluate((el) => document.activeElement === el), true);
    const linkEvidence = await activationLink.evaluate((link) => {
      window.__t03Activation = null;
      link.addEventListener('click', (event) => {
        window.__t03Activation = { href: link.href, detail: event.detail };
        event.preventDefault();
      }, { capture: true, once: true });
      return { href: link.href, target: link.target, rel: link.rel };
    });
    assert.equal(new URL(linkEvidence.href).protocol, 'https:');
    assert.equal(linkEvidence.target, '_blank');
    assert(linkEvidence.rel.includes('noopener'));
    await page.keyboard.press('Enter');
    assert.deepEqual(await page.evaluate(() => window.__t03Activation), {
      href: linkEvidence.href,
      detail: 0,
    });
  });

  await check('viewport-narrow', async () => {
    await searchByQuery(page, 'valkey', { categories: 'general' });
    await page.setViewportSize({ width: 390, height: 844 });
    assert.equal(await viewportObservation(page, 'narrow-390x844'), true);
    await screenshot(page, 'contract-narrow-390x844.png');
  });

  await check('viewport-320css', async () => {
    await page.setViewportSize({ width: 320, height: 640 });
    assert.equal(await viewportObservation(page, 'narrow-320x640-css-pixels'), true);
    await screenshot(page, 'contract-320css.png');
    const longResponse = await searchByQuery(page, 'long', { categories: 'general' });
    assert.equal(longResponse.status(), 200);
    assert.equal(await viewportObservation(page, 'long-content-320css'), true);
    await screenshot(page, 'contract-long-content-320css.png');
  });

  await check('reduced-motion', async () => {
    const reducedContext = await newContext({
      viewport: { width: 390, height: 844 },
      reducedMotion: 'reduce',
      initScript: `(() => {
        window.__t03ScrollBehaviors = [];
        const original = Element.prototype.scrollIntoView;
        Element.prototype.scrollIntoView = function (options) {
          window.__t03ScrollBehaviors.push(options?.behavior ?? 'auto');
          return original.call(this, options);
        };
      })();`,
    });
    const reducedPage = await reducedContext.newPage();
    attachPageEvidence(reducedPage);
    await navigate(reducedPage, '/search?q=valkey&categories=general');
    assert.equal(await reducedPage.evaluate(() => matchMedia('(prefers-reduced-motion: reduce)').matches), true);
    await reducedPage.locator('a.brand').focus();
    await reducedPage.keyboard.press('j');
    assert.equal(await reducedPage.evaluate(() => window.__t03ScrollBehaviors.at(-1)), 'auto');
    await screenshot(reducedPage, 'contract-reduced-motion.png');
    await reducedContext.close();
  });

  assert(landingMetric, 'landing budget sample was not captured');
  assert(resultsMetric, 'results budget sample was not captured');
  await collectContractBudgets(landingMetric, resultsMetric, page);

  await context.close();
}

async function runErrorScenario(id, expectedStatus, query) {
  const context = await newContext({ viewport: { width: 1440, height: 900 } });
  const page = await context.newPage();
  attachPageEvidence(page);
  await check(id, async () => {
    const response = await searchByQuery(page, query, { categories: 'general' });
    assert.equal(response.status(), expectedStatus);
    assert(await page.getByRole('alert').isVisible());
    assert.equal(await page.getByRole('searchbox', { name: 'Search SlopSearX' }).inputValue(), query);
    assert.equal(await page.locator('details.scope-panel select[name="categories"]').inputValue(), 'general');
    await screenshot(page, `${scenario}-error.png`);
  });
  await context.close();
}

async function runUnavailable() {
  const context = await newContext({ viewport: { width: 1440, height: 900 } });
  const page = await context.newPage();
  attachPageEvidence(page);
  await check('unavailable-source', async () => {
    const response = await searchByQuery(page, 'valkey', { categories: 'general' });
    assert.equal(response.status(), 503);
    assert(await page.getByRole('alert').getByText('No configured source returned a result.').isVisible());
    assert.equal(await page.locator('article[data-result-card]').count(), 0);
    assert.equal(await page.getByRole('searchbox', { name: 'Search SlopSearX' }).inputValue(), 'valkey');
    assert.equal(await page.locator('details.scope-panel select[name="categories"]').inputValue(), 'general');
    assert((await page.locator('body').innerText()).includes('0 of 2 sources answered'));
    await screenshot(page, 'unavailable-source.png');
  });
  await context.close();
}

async function writeRuntimeEvidence() {
  const path = pathFor(`${scenario}.browser-runtime-errors.json`);
  const document = {
    unexpected_requests: unexpectedRequests,
    console_errors: consoleErrors,
    http_error_responses: httpErrorResponses,
    http_error_console_statuses: httpErrorConsoleStatuses,
    page_errors: pageErrors,
    request_failures: requestFailures,
  };
  const bytes = Buffer.from(`${JSON.stringify(document, null, 2)}\n`);
  writeFileSync(path, bytes, { flag: 'wx' });
  artifacts.push({ path, sha256: sha256(bytes), bytes: bytes.byteLength, kind: 'runtime-errors', width: null, height: null });

  const viewportPath = pathFor(`${scenario}.browser-viewports.json`);
  const viewportDocument = {
    observations: viewportHistory,
    actual_200_percent_browser_zoom: 'not tested; CSS viewport and device-pixel ratio are not zoom emulation',
  };
  const viewportBytes = Buffer.from(`${JSON.stringify(viewportDocument, null, 2)}\n`);
  writeFileSync(viewportPath, viewportBytes, { flag: 'wx' });
  artifacts.push({ path: viewportPath, sha256: sha256(viewportBytes), bytes: viewportBytes.byteLength, kind: 'viewport-observations', width: null, height: null });
}

async function main() {
  const scenarioValue = process.env.T03_SCENARIO;
  const baseValue = process.env.T03_BROWSER_BASE_URL;
  const evidenceValue = process.env.T03_EVIDENCE_DIR;
  assert(supportedScenarios.has(scenarioValue), `T03_SCENARIO must be one of ${[...supportedScenarios].join(', ')}`);
  assert(baseValue, 'T03_BROWSER_BASE_URL is required');
  assert(evidenceValue && isAbsolute(evidenceValue), 'T03_EVIDENCE_DIR must be an absolute path');
  const parsedBase = new URL(baseValue);
  assert.equal(parsedBase.protocol, 'http:');
  assert(['127.0.0.1', 'localhost', '::1'].includes(parsedBase.hostname));
  assert(parsedBase.port, 'fixture URL must include its ephemeral loopback port');
  assert(['', '/'].includes(parsedBase.pathname));
  assert.equal(parsedBase.search, '');
  assert.equal(parsedBase.hash, '');
  const evidenceStat = lstatSync(evidenceValue);
  assert(evidenceStat.isDirectory() && !evidenceStat.isSymbolicLink(), 'evidence directory must be a real directory');
  scenario = scenarioValue;
  baseUrl = parsedBase.origin;
  evidenceDirectory = resolve(evidenceValue);

  const packageData = JSON.parse(readFileSync(packagePath, 'utf8'));
  browserInfo.playwright_version = packageData.devDependencies['playwright-core'];
  browser = await chromium.launch({ headless: true });
  browserInfo.version = browser.version();

  if (scenario === 'contract') await runContract();
  else if (scenario === 'cached-error') await runErrorScenario('cached-error', 503, 'keep-cached');
  else if (scenario === 'rate-limited') await runErrorScenario('rate-limited', 429, 'keep-rate-limited');
  else if (scenario === 'html-disabled') await runErrorScenario('disabled-format', 403, 'keep-disabled-format');
  else if (scenario === 'unavailable') await runUnavailable();

  try {
    await browser.close();
    browserClosed = true;
  } catch (error) {
    pageErrors.push(`Browser shutdown failed: ${error instanceof Error ? error.message : String(error)}`);
  }
  if (unexpectedRequests.length > 0) addFailure('network-policy', 'browser attempted an outbound request');
  if (consoleErrors.length > 0) addFailure('console-errors', 'browser emitted console errors');
  const expectedHttpErrors = {
    contract: [400, 403, 503],
    'cached-error': [503],
    'rate-limited': [429],
    'html-disabled': [403],
    unavailable: [503],
  }[scenario];
  const observedHttpStatuses = httpErrorResponses.map((row) => row.status).sort((a, b) => a - b);
  const observedConsoleStatuses = [...httpErrorConsoleStatuses].sort((a, b) => a - b);
  const expectedSorted = [...expectedHttpErrors].sort((a, b) => a - b);
  if (JSON.stringify(observedHttpStatuses) !== JSON.stringify(expectedSorted)) {
    addFailure('http-status-inventory', `observed ${JSON.stringify(observedHttpStatuses)}; expected ${JSON.stringify(expectedSorted)}`);
  }
  if (JSON.stringify(observedConsoleStatuses) !== JSON.stringify(expectedSorted)) {
    addFailure('http-status-console-parity', `observed ${JSON.stringify(observedConsoleStatuses)}; expected ${JSON.stringify(expectedSorted)}`);
  }
  if (pageErrors.length > 0) addFailure('page-errors', 'browser emitted uncaught page errors or crashed');
  if (requestFailures.length > 0) addFailure('request-failures', 'browser request failed');
  if (!browserClosed) addFailure('browser-closed', 'browser did not close cleanly');

  const required = requiredByScenario[scenario];
  const missing = required.filter((id) => !completed.includes(id));
  const unexpected = completed.filter((id) => !required.includes(id));
  if (missing.length > 0) addFailure('scenario-completeness', `missing IDs: ${missing.join(', ')}`);
  if (unexpected.length > 0) addFailure('scenario-completeness', `unexpected IDs: ${unexpected.join(', ')}`);
  if (completed.length !== required.length) addFailure('scenario-completeness', `expected ${required.length} completed IDs; got ${completed.length}`);

  await writeRuntimeEvidence();
  const receiptPath = pathFor(`${scenario}.browser-receipt.json`);
  const clientHash = sha256(readFileSync(sourcePath));
  const receipt = {
    schema: 'T03_BROWSER_V1',
    scenario,
    base_url: baseUrl,
    client_sha256: clientHash,
    browser: browserInfo,
    completed: requiredByScenario[scenario].filter((id) => completed.includes(id)),
    skipped: [],
    failures,
    unexpected_requests: unexpectedRequests,
    console_errors: consoleErrors,
    http_error_responses: httpErrorResponses,
    http_error_console_statuses: httpErrorConsoleStatuses,
    page_errors: pageErrors,
    browser_closed: browserClosed,
    artifacts,
    budgets,
  };
  const receiptBytes = Buffer.from(`${JSON.stringify(receipt, null, 2)}\n`);
  writeFileSync(receiptPath, receiptBytes, { flag: 'wx' });
  process.stdout.write(`T03_BROWSER_COMPLETION ${JSON.stringify({ scenario, receipt: receiptPath, sha256: sha256(receiptBytes) })}\n`);
  if (failures.length > 0) process.exitCode = 1;
}

main().catch(async (error) => {
  if (browser) {
    try {
      await browser.close();
      browserClosed = true;
    } catch (closeError) {
      pageErrors.push(`Browser shutdown failed: ${closeError instanceof Error ? closeError.message : String(closeError)}`);
    }
  }
  const message = error instanceof Error ? error.message : String(error);
  if (!scenario) scenario = process.env.T03_SCENARIO ?? 'unknown';
  if (!baseUrl) baseUrl = process.env.T03_BROWSER_BASE_URL ?? '';
  if (!evidenceDirectory && process.env.T03_EVIDENCE_DIR) evidenceDirectory = resolve(process.env.T03_EVIDENCE_DIR);
  failures.push(`startup: ${message}`);
  if (evidenceDirectory && isAbsolute(evidenceDirectory) && existsSync(evidenceDirectory)) {
    try {
      await writeRuntimeEvidence();
      const receiptPath = pathFor(`${scenario}.browser-receipt.json`);
      const receipt = {
        schema: 'T03_BROWSER_V1',
        scenario,
        base_url: baseUrl,
        client_sha256: sha256(readFileSync(sourcePath)),
        browser: browserInfo,
        completed: requiredByScenario[scenario]?.filter((id) => completed.includes(id)) ?? completed,
        skipped: [],
        failures,
        unexpected_requests: unexpectedRequests,
        console_errors: consoleErrors,
        page_errors: pageErrors,
        browser_closed: browserClosed,
        artifacts,
        budgets,
      };
      const bytes = Buffer.from(`${JSON.stringify(receipt, null, 2)}\n`);
      writeFileSync(receiptPath, bytes, { flag: 'wx' });
      process.stdout.write(`T03_BROWSER_COMPLETION ${JSON.stringify({ scenario, receipt: receiptPath, sha256: sha256(bytes) })}\n`);
    } catch (receiptError) {
      process.stderr.write(`T03 browser receipt could not be written: ${receiptError instanceof Error ? receiptError.message : String(receiptError)}\n`);
    }
  }
  process.exitCode = 1;
});
