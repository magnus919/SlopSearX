import assert from 'node:assert/strict';
import { measureInteraction, timingControls, INTERACTION_DEFINITION } from './interaction.mjs';
import { createHash } from 'node:crypto';
import { readFileSync, writeFileSync, lstatSync, existsSync, realpathSync, statSync } from 'node:fs';
import { dirname, isAbsolute, join, relative, resolve, sep } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { gzipSync } from 'node:zlib';
const here = dirname(fileURLToPath(import.meta.url));
const sourcePath = fileURLToPath(import.meta.url);
const packagePath = join(here, 'package.json');
const packageData = JSON.parse(readFileSync(packagePath, 'utf8'));
const lockData = JSON.parse(readFileSync(join(here, 'package-lock.json'), 'utf8'));
const lockedPlaywright = lockData.packages?.['node_modules/playwright-core'];
const lockedPlaywrightVersion = lockedPlaywright?.version;
assert.equal(
  packageData.devDependencies?.['playwright-core'],
  lockedPlaywrightVersion,
  'Playwright manifest differs from lockfile',
);
assert.equal(typeof lockedPlaywrightVersion, 'string', 'Playwright lockfile version is missing');

const hasPlaywrightOverride = Object.hasOwn(process.env, 'T03_PLAYWRIGHT_CORE_MODULE');
const playwrightOverride = process.env.T03_PLAYWRIGHT_CORE_MODULE;
assert(
  !hasPlaywrightOverride || (typeof playwrightOverride === 'string' && playwrightOverride.length > 0),
  'T03_PLAYWRIGHT_CORE_MODULE must not be empty',
);
assert(!hasPlaywrightOverride || isAbsolute(playwrightOverride), 'T03_PLAYWRIGHT_CORE_MODULE must be absolute');
const selectedPlaywrightPath = hasPlaywrightOverride
  ? playwrightOverride
  : fileURLToPath(import.meta.resolve('playwright-core'));
const playwrightModulePath = realpathSync(selectedPlaywrightPath);
assert(statSync(playwrightModulePath).isFile(), 'selected Playwright entrypoint must be a regular file');
function locatePlaywrightPackage(modulePath) {
  let directory = dirname(modulePath);
  while (true) {
    const manifestPath = join(directory, 'package.json');
    if (existsSync(manifestPath)) {
      const metadata = JSON.parse(readFileSync(manifestPath, 'utf8'));
      if (metadata.name === 'playwright-core') {
        return { directory, metadata };
      }
    }
    const parent = dirname(directory);
    if (parent === directory) break;
    directory = parent;
  }
  throw new Error('selected module is not inside the playwright-core package');
}
const playwrightPackage = locatePlaywrightPackage(playwrightModulePath);
assert.equal(playwrightPackage.metadata.version, lockedPlaywrightVersion, 'loaded Playwright package differs from lockfile');
const playwrightImportEntry = playwrightPackage.metadata.exports?.['.']?.import;
assert.equal(playwrightImportEntry, './index.mjs', 'playwright-core ESM export differs from the locked release contract');
assert(!isAbsolute(playwrightImportEntry), 'playwright-core ESM entry must be package-relative');
const resolvedPlaywrightEntry = resolve(playwrightPackage.directory, playwrightImportEntry);
const relativePlaywrightEntry = relative(playwrightPackage.directory, resolvedPlaywrightEntry);
assert(
  relativePlaywrightEntry && relativePlaywrightEntry !== '..' && !relativePlaywrightEntry.startsWith(`..${sep}`),
  'playwright-core ESM entry escapes its package directory',
);
const canonicalPlaywrightRoot = realpathSync(playwrightPackage.directory);
const canonicalPlaywrightPath = realpathSync(resolvedPlaywrightEntry);
const canonicalPlaywrightRelative = relative(canonicalPlaywrightRoot, canonicalPlaywrightPath);
assert(
  canonicalPlaywrightRelative &&
    canonicalPlaywrightRelative !== '..' &&
    !canonicalPlaywrightRelative.startsWith(`..${sep}`),
  'playwright-core ESM entrypoint escapes its package root',
);
assert(statSync(canonicalPlaywrightPath).isFile(), 'playwright-core ESM entrypoint must be a regular file');
assert.equal(
  playwrightModulePath,
  canonicalPlaywrightPath,
  'selected module is not the locked playwright-core ESM entrypoint',
);
const { chromium, firefox, webkit } = await import(pathToFileURL(canonicalPlaywrightPath).href);
const supportedScenarios = new Set([
  'contract',
  'cached-error',
  'rate-limited',
  'html-disabled',
  'unavailable',
  'source-availability',
]);
const requiredByScenario = {
  contract: [
    'results-normal',
    'optional-metadata',
    'empty-date-controls',
    'date-controls-roundtrip',
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
  'source-availability': ['source-availability'],
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
const syntheticFixtureResponses = [];
const artifacts = [];
const interactionSamples = [];
const profiledBudgetPages = new WeakSet();
let contractBudgetProfileRequired = false;
let interactionControls = null;
const handoffSamples = [];
let browser = null;
let browserClosed = false;
let browserInfo = { name: 'chromium', version: '', playwright_version: '' };
let browserEngine = 'chromium';
let engineConsoleParity = 'not-captured';
let budgets = {};
let evidenceDirectory = '';
let baseUrl = '';
let scenario = '';
let dateRoundtripEvidence = null;
let searchSummaryVisible = null;

function sha256(bytes) {
  return createHash('sha256').update(bytes).digest('hex');
}

function addFailure(id, error) {
  const message = error instanceof Error ? error.message : String(error);
  failures.push(`${id}: ${message}`);
}

function assertBudgetProfiled(page, required, profiledPages = profiledBudgetPages) {
  if (required) assert(profiledPages.has(page), 'Chromium contract interaction came from an unprofiled page');
}

function verifyBudgetProfileGuard() {
  const page = {};
  const profiledPages = new WeakSet();
  assert.throws(() => assertBudgetProfiled(page, true, profiledPages), /unprofiled page/);
  profiledPages.add(page);
  assert.doesNotThrow(() => assertBudgetProfiled(page, true, profiledPages));
  assert.doesNotThrow(() => assertBudgetProfiled({}, false, profiledPages));
}
verifyBudgetProfileGuard();

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
    if (
      scenario === 'contract' &&
      requestUrl === 'https://cdn.example.com/thumb.jpg' &&
      route.request().method() === 'GET'
    ) {
      const body = '<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"></svg>';
      await route.fulfill({ status: 200, contentType: 'image/svg+xml', body });
      syntheticFixtureResponses.push({
        url: requestUrl,
        status: 200,
        content_type: 'image/svg+xml',
        purpose: 'offline T03 media thumbnail',
      });
      return;
    }
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

async function navigateWithoutBudgetSample(page, action) {
  const navigation = page.waitForNavigation({ waitUntil: 'domcontentloaded' });
  await action();
  const response = await navigation;
  assert(response, 'form or link activation did not produce a navigation response');
  return response;
}

function oneFormValue(rows, name) {
  const values = rows.filter((row) => row.name === name).map((row) => row.value);
  assert.equal(values.length, 1, 'expected one FormData value for ' + name);
  return values[0];
}

async function roundtripControls(page) {
  return page.evaluate(() => {
    const dateFrom = document.querySelector('details.scope-panel input[name="date_from"]');
    const dateTo = document.querySelector('details.scope-panel input[name="date_to"]');
    return {
      query: document.querySelector('#portal-query')?.value ?? null,
      date_from: dateFrom?.value ?? null,
      date_to: dateTo?.value ?? null,
      date_from_type: dateFrom?.type ?? null,
      date_to_type: dateTo?.type ?? null,
      date_from_valid: dateFrom?.checkValidity() ?? false,
      date_to_valid: dateTo?.checkValidity() ?? false,
    };
  });
}

async function roundtripHtmlProof(page) {
  return page.evaluate(() => {
    const row = (name) => {
      const element = document.querySelector('[data-filter="' + name + '"]');
      return element ? { status: element.dataset.status, text: element.innerText } : null;
    };
    return { date_from: row('date_from'), date_to: row('date_to') };
  });
}

async function recordRoundtripForm(page, id, selector, expected, action) {
  const sourcePage = Number(new URL(page.url()).searchParams.get('pageno') || '1');
  assert(Number.isSafeInteger(sourcePage) && sourcePage > 0, id + ' source page is invalid');
  const form = page.locator(selector);
  const formData = await form.evaluate((element) =>
    Array.from(new FormData(element).entries()).map(([name, value]) => ({ name, value: String(value) })),
  );
  assert.equal(oneFormValue(formData, 'q'), expected.query);
  assert.equal(oneFormValue(formData, 'engines'), 'openalex');
  assert.equal(oneFormValue(formData, 'date_from'), expected.date_from ?? '');
  assert.equal(oneFormValue(formData, 'date_to'), expected.date_to ?? '');

  const response = await navigateWithoutBudgetSample(page, action);
  assert.equal(response.status(), 200, id);
  const requestUrl = response.request().url();
  const request = new URL(requestUrl);
  assert.equal(request.pathname, '/search');
  assert.equal(request.searchParams.get('q'), expected.query);
  assert.equal(request.searchParams.get('engines'), 'openalex');
  const dateValue = (name) => {
    const value = request.searchParams.get(name);
    return value === '' ? null : value;
  };
  assert.equal(dateValue('date_from'), expected.date_from ?? null);
  assert.equal(dateValue('date_to'), expected.date_to ?? null);
  assert.equal(Number(request.searchParams.get('pageno') || '1'), 1);

  const controls = await roundtripControls(page);
  assert.equal(controls.query, expected.query);
  assert.equal(controls.date_from, expected.date_from ?? '');
  assert.equal(controls.date_to, expected.date_to ?? '');
  assert.equal(controls.date_from_type, 'date');
  assert.equal(controls.date_to_type, 'date');
  assert.equal(controls.date_from_valid, true);
  assert.equal(controls.date_to_valid, true);
  const htmlProof = await roundtripHtmlProof(page);
  for (const [field, value] of [['date_from', expected.date_from], ['date_to', expected.date_to]]) {
    if (value) {
      assert.equal(htmlProof[field]?.status, 'enforced', id + ' proof status for ' + field);
      assert(htmlProof[field].text.includes(value), id + ' proof value for ' + field);
    }
  }
  return {
    id,
    kind: 'form',
    source_page: sourcePage,
    form_data: formData,
    submitted_url: requestUrl,
    final_url: page.url(),
    status: response.status(),
    controls,
    html_proof: htmlProof,
  };
}

async function recordRoundtripLink(page, id, locator, expected) {
  const href = await locator.getAttribute('href');
  assert(href, id + ' link href missing');
  const response = await navigateWithoutBudgetSample(page, () => locator.click());
  assert.equal(response.status(), 200, id);
  const requestUrl = response.request().url();
  const request = new URL(requestUrl);
  assert.equal(request.pathname, '/search');
  assert.equal(request.searchParams.get('q'), expected.query);
  assert.equal(request.searchParams.get('engines'), 'openalex');
  assert.equal(request.searchParams.get('date_from'), expected.date_from);
  assert.equal(request.searchParams.get('date_to'), expected.date_to);
  assert.equal(Number(request.searchParams.get('pageno') || '1'), expected.page);
  assert.equal(request.searchParams.get('format'), expected.format === 'html' ? null : expected.format);
  const contentType = response.headers()['content-type'] ?? '';
  const mediaType = contentType.split(';', 1)[0].trim().toLowerCase();
  const bodyBytes = Buffer.from(await response.body());
  let proof = null;
  let controls = null;
  let bodyPath = null;
  if (expected.format === 'json' || expected.format === 'yaml') {
    bodyPath = pathFor(`contract.${id}.response.${expected.format}`);
    writeFileSync(bodyPath, bodyBytes, { flag: 'wx' });
    artifacts.push({
      path: bodyPath,
      sha256: sha256(bodyBytes),
      bytes: bodyBytes.byteLength,
      kind: 'date-roundtrip-machine-response',
      content_type: contentType,
      width: null,
      height: null,
    });
  }
  if (expected.format === 'json') {
    assert.equal(mediaType, 'application/json');
    const document = JSON.parse(bodyBytes.toString('utf8'));
    proof = document?.meta?.date_filter;
    assert.equal(proof?.status, 'enforced');
    assert.equal(proof?.requested?.date_from, expected.date_from);
    assert.equal(proof?.requested?.date_to, expected.date_to);
    assert.equal(proof?.applied?.date_from, expected.date_from);
    assert.equal(proof?.applied?.date_to, expected.date_to);
    assert(proof?.enforced_by?.includes('upstream:openalex'));
  } else if (expected.format === 'yaml') {
    assert.equal(mediaType, 'text/vnd.yaml+markdown');
  } else {
    assert.equal(mediaType, 'text/html');
    controls = await roundtripControls(page);
    assert.equal(controls.query, expected.query);
    assert.equal(controls.date_from, expected.date_from);
    assert.equal(controls.date_to, expected.date_to);
    assert.equal(controls.date_from_type, 'date');
    assert.equal(controls.date_to_type, 'date');
    assert.equal(controls.date_from_valid, true);
    assert.equal(controls.date_to_valid, true);
    proof = await roundtripHtmlProof(page);
    assert.equal(proof.date_from?.status, 'enforced');
    assert.equal(proof.date_to?.status, 'enforced');
  }
  return {
    id,
    kind: 'link',
    href: new URL(href, baseUrl).href,
    request_url: requestUrl,
    final_url: page.url(),
    status: response.status(),
    format: expected.format,
    content_type: contentType,
    body_path: bodyPath,
    controls,
    proof,
    body_sha256: sha256(bodyBytes),
    body_bytes: bodyBytes.byteLength,
  };
}

async function searchByQuery(page, query, extra = {}) {
  const params = new URLSearchParams({ q: query, ...extra });
  return navigate(page, `/search?${params.toString()}`);
}

async function clickAndMeasure(page, locator, id) {
  assertBudgetProfiled(page, contractBudgetProfileRequired);
  const sample = await measureInteraction(page, () => locator.click(), {
    event: 'click', target: '[data-theme-toggle]', selector: 'html',
    kind: 'attribute', attribute: 'data-theme', value: 'darker',
  });
  interactionSamples.push({ id, ...sample });
}

async function keyboardAndMeasure(page, key) {
  assertBudgetProfiled(page, contractBudgetProfileRequired);
  const expected = key === 'Enter'
    ? { event: 'keydown', key, target: 'summary', selector: 'details.result-explanation', kind: 'attribute', attribute: 'open', value: '' }
    : { event: 'keydown', key, selector: '.result-link', kind: 'focus' };
  const sample = await measureInteraction(page, () => page.keyboard.press(key), expected);
  interactionSamples.push({ id: key === 'Enter' ? 'disclosure' : 'focus', ...sample });
}

async function applyContractProfile(context, page, throttled) {
  if (!throttled) return;
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
  profiledBudgetPages.add(page);
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
  const interactionP95 = percentile95(interactionSamples.map((sample) => sample.duration_ms));
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
    interaction_evidence: { definition: INTERACTION_DEFINITION, samples: interactionSamples, controls: interactionControls },
    handoff_p95_ms: metric(handoffP95, 150),
  };
  for (const [name, result] of Object.entries(budgets)) {
    if (name === 'profile' || name === 'cls_samples' || name === 'interaction_evidence') continue;
    if (!result.pass) addFailure(`budget-${name}`, `observed ${result.observed}, limit ${result.limit}`);
  }
}

async function runContract() {
  interactionControls = await timingControls(browser);
  const context = await newContext({ viewport: { width: 1440, height: 900 } });
  const page = await context.newPage();
  attachPageEvidence(page);

  // Chromium profiles the client budgets under CDP Fast 3G and 4x CPU throttling.
  // Other engines have no CDP; they run the same journeys without throttling and
  // do not claim the Chromium budget profile.
  const throttled = browserEngine === 'chromium';
  contractBudgetProfileRequired = throttled;
  await applyContractProfile(context, page, throttled);
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
    assert(await page.getByRole('heading', { name: 'Search SlopSearX.', exact: true }).isVisible());
    assert(await page.getByRole('searchbox', { name: 'Search SlopSearX', exact: true }).isVisible());
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
    const cards = page.locator('article[data-result-card]');
    assert.equal(await cards.count(), 1);
    const article = cards.first();
    const resultLink = article.getByRole('link', {
      name: 'A useful deterministic result',
      exact: true,
    });
    assert(await resultLink.isVisible());
    assert.equal(await resultLink.getAttribute('href'), 'https://example.org/article?source=fixture');
    const consensus = article.locator('.result-pill.consensus');
    assert(await consensus.isVisible());
    assert.equal(await consensus.innerText(), 'MATCHED 2 SOURCES');
    const sourcePills = article.locator('.result-meta .result-pill');
    assert.deepEqual(await sourcePills.allTextContents(), ['alpha', 'Beta <source>']);
    assert(await sourcePills.nth(0).isVisible());
    assert(await sourcePills.nth(1).isVisible());
    const openResult = article.getByRole('link', { name: 'Open result ↗', exact: true });
    assert(await openResult.isVisible());
    assert.equal(await openResult.getAttribute('href'), 'https://example.org/article?source=fixture');
    const explanation = article.locator('details.result-explanation');
    await explanation.locator('summary').click();
    assert.deepEqual(
      await explanation.locator('.explanation-sources .result-pill').allTextContents(),
      ['alpha', 'Beta <source>'],
    );
    assert(await explanation.getByText('structurally eligible', { exact: true }).isVisible());
    const summary = page.getByRole('complementary', { name: 'Search summary' });
    searchSummaryVisible = await summary.isVisible();
    assert.equal(searchSummaryVisible, true);
    assert(await summary.getByText('Source status', { exact: true }).isVisible());
    const jsonView = page.getByRole('link', { name: 'Open JSON view ↗', exact: true });
    assert(await jsonView.isVisible());
    const machineView = new URL(await jsonView.getAttribute('href'), page.url());
    assert.equal(machineView.origin, new URL(baseUrl).origin);
    assert.equal(machineView.pathname, '/search');
    assert.equal(machineView.searchParams.get('format'), 'json');
    assert.equal(machineView.searchParams.get('q'), 'valkey');
    assert.equal(machineView.searchParams.get('categories'), 'general');
    const scope = page.getByRole('combobox', { name: 'Scope', exact: true });
    assert(await scope.isVisible());
    assert.equal(await scope.inputValue(), 'general');
    assert(await page.getByRole('button', { name: 'Apply filters', exact: true }).isVisible());
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

  await check('empty-date-controls', async () => {
    const response = await searchByQuery(page, 'unbounded', { engines: 'openalex' });
    assert.equal(response.status(), 200);

    for (const name of ['date_from', 'date_to']) {
      const control = page.locator(`input[name="${name}"][type="date"]`);
      assert.equal(await control.inputValue(), '');
      assert.equal(await control.evaluate((input) => input.defaultValue), '');
      assert.equal(await control.getAttribute('value'), '');
    }

    const filtersForm = page.locator('form').filter({ has: page.getByRole('button', { name: 'Apply filters' }) });
    const dateEntries = await filtersForm.evaluate((form) =>
      [...new FormData(form).entries()].filter(([name]) => ['date_from', 'date_to'].includes(name)),
    );
    assert.deepEqual(dateEntries, [['date_from', ''], ['date_to', '']]);
    await screenshot(page, 'contract-empty-date-controls.png');

    const submitted = await navigateBy(page, () => page.getByRole('button', { name: 'Apply filters' }).click());
    assert.equal(submitted.status(), 200);
    const submittedUrl = new URL(page.url());
    for (const name of ['date_from', 'date_to']) {
      const value = submittedUrl.searchParams.get(name);
      assert(value === null || value === '', `${name} must remain unbounded when untouched`);
    }
  });

  await check('date-controls-roundtrip', async () => {
    const observations = [];
    const navigationRequests = [];
    const requestListener = (request) => {
      if (!request.isNavigationRequest()) return;
      try {
        const url = new URL(request.url());
        const query = url.searchParams.get('q') ?? '';
        if (url.origin === new URL(baseUrl).origin && query.startsWith('t03-roundtrip-')) {
          navigationRequests.push(request.url());
        }
      } catch {
        unexpectedRequests.push(request.url());
      }
    };
    page.on('request', requestListener);
    let seed;
    try {
      const seedResponse = await searchByQuery(page, 't03-roundtrip-browser', { engines: 'openalex' });
      assert.equal(seedResponse.status(), 200);
      const seedControls = await roundtripControls(page);
      assert.equal(seedControls.query, 't03-roundtrip-browser');
      assert.equal(seedControls.date_from, '');
      assert.equal(seedControls.date_to, '');
      seed = {
        status: seedResponse.status(),
        url: seedResponse.request().url(),
        controls: seedControls,
      };

      const scopeForm = 'details.scope-panel form';
      const dateFrom = page.locator('details.scope-panel input[name="date_from"]');
      const dateTo = page.locator('details.scope-panel input[name="date_to"]');
      const submitScope = () => page.locator('details.scope-panel button.scope-apply').click();

      observations.push(await recordRoundtripForm(page, 'blank-submit', scopeForm, {
        query: 't03-roundtrip-browser', date_from: null, date_to: null,
      }, submitScope));

      await dateFrom.fill('2024-02-29');
      await dateTo.fill('');
      observations.push(await recordRoundtripForm(page, 'lower-only', scopeForm, {
        query: 't03-roundtrip-browser', date_from: '2024-02-29', date_to: null,
      }, submitScope));

      await dateFrom.fill('');
      await dateTo.fill('2024-02-29');
      observations.push(await recordRoundtripForm(page, 'upper-only', scopeForm, {
        query: 't03-roundtrip-browser', date_from: null, date_to: '2024-02-29',
      }, submitScope));

      await dateFrom.fill('2024-02-29');
      await dateTo.fill('2024-02-29');
      observations.push(await recordRoundtripForm(page, 'equal-leap-day', scopeForm, {
        query: 't03-roundtrip-browser', date_from: '2024-02-29', date_to: '2024-02-29',
      }, submitScope));

      observations.push(await recordRoundtripLink(
        page,
        'next-page-before-query-refinement',
        page.locator('a[aria-label="Try next result page"]'),
        { query: 't03-roundtrip-browser', date_from: '2024-02-29', date_to: '2024-02-29', page: 2, format: 'html' },
      ));

      const queryInput = page.locator('form.search-form input[name="q"]');
      await queryInput.fill('t03-roundtrip-refined');
      observations.push(await recordRoundtripForm(page, 'query-refinement', 'form.search-form', {
        query: 't03-roundtrip-refined', date_from: '2024-02-29', date_to: '2024-02-29',
      }, () => page.locator('form.search-form button.search-button').click()));

      observations.push(await recordRoundtripLink(
        page,
        'next-page',
        page.locator('a[aria-label="Try next result page"]'),
        { query: 't03-roundtrip-refined', date_from: '2024-02-29', date_to: '2024-02-29', page: 2, format: 'html' },
      ));

      await dateTo.fill('2024-03-01');
      observations.push(await recordRoundtripForm(page, 'scope-refinement', scopeForm, {
        query: 't03-roundtrip-refined', date_from: '2024-02-29', date_to: '2024-03-01',
      }, submitScope));
      await dateTo.fill('2024-02-29');
      observations.push(await recordRoundtripForm(page, 'scope-restoration', scopeForm, {
        query: 't03-roundtrip-refined', date_from: '2024-02-29', date_to: '2024-02-29',
      }, submitScope));

      observations.push(await recordRoundtripLink(
        page,
        'next-page-after-scope',
        page.locator('a[aria-label="Try next result page"]'),
        { query: 't03-roundtrip-refined', date_from: '2024-02-29', date_to: '2024-02-29', page: 2, format: 'html' },
      ));
      observations.push(await recordRoundtripLink(
        page,
        'previous-page',
        page.locator('a[aria-label="Previous result page"]'),
        { query: 't03-roundtrip-refined', date_from: '2024-02-29', date_to: '2024-02-29', page: 1, format: 'html' },
      ));

      observations.push(await recordRoundtripLink(
        page,
        'json-link',
        page.getByRole('link', { name: 'Open JSON view ↗' }),
        { query: 't03-roundtrip-refined', date_from: '2024-02-29', date_to: '2024-02-29', page: 1, format: 'json' },
      ));
      await page.goBack({ waitUntil: 'domcontentloaded' });
      await page.locator('details.scope-panel').waitFor();

      observations.push(await recordRoundtripLink(
        page,
        'yaml-link',
        page.getByRole('link', { name: 'Open YAML view ↗' }),
        { query: 't03-roundtrip-refined', date_from: '2024-02-29', date_to: '2024-02-29', page: 1, format: 'yaml' },
      ));
      await page.goBack({ waitUntil: 'domcontentloaded' });
      await page.locator('details.scope-panel').waitFor();

      const clearFrom = page.locator('details.scope-panel input[name="date_from"]');
      const clearTo = page.locator('details.scope-panel input[name="date_to"]');
      await clearFrom.fill('');
      await clearTo.fill('');
      observations.push(await recordRoundtripForm(page, 'clear-bounds', scopeForm, {
        query: 't03-roundtrip-refined', date_from: null, date_to: null,
      }, submitScope));

      const clearedLinks = await page.locator('a.rail-link').evaluateAll((elements) =>
        elements.map((element) => {
          const url = new URL(element.href);
          return {
            format: url.searchParams.get('format'),
            href: element.href,
            date_from_present: url.searchParams.has('date_from'),
            date_to_present: url.searchParams.has('date_to'),
          };
        }),
      );
      assert.deepEqual(clearedLinks.map((link) => link.format).sort(), ['json', 'yaml']);
      assert(clearedLinks.every((link) => !link.date_from_present && !link.date_to_present));
      observations.push({ id: 'cleared-machine-links', kind: 'link-inventory', links: clearedLinks });
    } finally {
      page.off('request', requestListener);
    }
    dateRoundtripEvidence = {
      schema: 'T03_DATE_ROUNDTRIP_V1',
      seed,
      navigation_requests: navigationRequests,
      observations,
    };
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
    assert.equal(await page.locator('.rail-source-unavailable .rail-source-name').innerText(), 'Beta <source>');

    const emptySource = await searchByQuery(page, 'empty-source', { categories: 'general' });
    assert.equal(emptySource.status(), 200);
    assert(await page.getByRole('status').getByText('No results from some sources.').isVisible());
    assert.equal(await page.getByRole('status').getByText('Some sources could not answer this search.').count(), 0);
    assert.equal(await page.locator('.rail-source-empty .rail-source-name').innerText(), 'Beta <source>');
    assert.equal(await page.locator('.rail-source-empty .rail-source-count').innerText(), 'no results');

    const media = await searchByQuery(page, 'media', { categories: 'general' });
    assert.equal(media.status(), 200);
    const mediaCard = page.locator('article[data-result-card]').filter({ hasText: 'image result' }).first();
    const special = mediaCard.locator('.result-special');
    assert.equal(await special.count(), 1);
    assert.deepEqual(await special.locator('span').allTextContents(), ['image result', 'security / vulnerability']);
    assert(await special.getByText('image result').isVisible());
    const thumbnail = mediaCard.locator('img');
    assert.equal(await thumbnail.getAttribute('src'), 'https://cdn.example.com/thumb.jpg');
    assert.equal(await thumbnail.getAttribute('alt'), 'Vulnerability result');
    assert(await mediaCard.locator('.result-special').getByText('security / vulnerability').isVisible());
    await thumbnail.scrollIntoViewIfNeeded();
    await thumbnail.evaluate(async (image) => {
      if (!image.complete) {
        await new Promise((resolve, reject) => {
          image.addEventListener('load', resolve, { once: true });
          image.addEventListener('error', () => reject(new Error('media thumbnail failed to load')), { once: true });
        });
      }
      if (image.naturalWidth <= 0) throw new Error('media thumbnail decoded to no image data');
    });
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
    const safeSearch = page.locator('select[name="safesearch"]');
    assert.equal(await safeSearch.getAttribute('aria-invalid'), 'true');
    assert.deepEqual((await safeSearch.getAttribute('aria-describedby')).match(/\S+/g), [
      'portal-error', 'strict-safesearch-note',
    ]);
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
    const previous = page.getByRole('link', { name: 'Previous result page', exact: true });
    const next = page.getByRole('link', { name: 'Try next result page', exact: true });
    for (const [link, expectedPage] of [[previous, '1'], [next, '3']]) {
      assert(await link.isVisible());
      const href = await link.getAttribute('href');
      assert(href);
      const target = new URL(href, page.url());
      assert.equal(target.origin, new URL(page.url()).origin);
      assert.equal(target.pathname, '/search');
      assert.equal(target.searchParams.get('q'), 'valkey');
      assert.equal(target.searchParams.get('categories'), 'general');
      assert.equal(target.searchParams.get('pageno'), expectedPage);
    }
    await navigateBy(page, () => previous.click());
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
    await clickAndMeasure(page, page.getByRole('button', { name: 'Use darker mode' }), 'theme');
    assert.equal(await page.locator('html').getAttribute('data-theme'), 'darker');
    assert(
      await page.getByRole('button', { name: 'Use dark mode', exact: true }).isVisible(),
      'switching to the darker theme must expose the Use dark mode button',
    );
    assert.equal(await page.evaluate(() => localStorage.getItem('slopsearx-theme')), 'darker');
    await page.reload({ waitUntil: 'domcontentloaded' });
    assert.equal(await page.locator('html').getAttribute('data-theme'), 'darker');

    const blocked = await newContext({
      viewport: { width: 390, height: 844 },
      initScript: `Object.defineProperty(window, 'localStorage', { configurable: true, get() { throw new Error('blocked by browser policy'); } });`,
    });
    const blockedPage = await blocked.newPage();
    attachPageEvidence(blockedPage);
    await applyContractProfile(blocked, blockedPage, throttled);
    await navigate(blockedPage, '/');
    assert.equal(await blockedPage.locator('html').getAttribute('data-theme'), 'dark');
    await clickAndMeasure(blockedPage, blockedPage.getByRole('button', { name: 'Use darker mode' }), 'theme-storage-blocked');
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
    const landingFocus = await input.evaluate((expected) => {
      const active = document.activeElement;
      return {
        focused: active === expected,
        readyState: document.readyState,
        activeTag: active instanceof Element ? active.tagName.toLowerCase() : null,
        activeId: active instanceof Element ? active.id : null,
        activeClasses: active instanceof Element ? [...active.classList] : [],
      };
    });
    assert.equal(landingFocus.focused, true, `landing searchbox autofocus: ${JSON.stringify(landingFocus)}`);
    await input.blur();
    assert.equal(await input.evaluate((element) => document.activeElement === element), false);
    await page.keyboard.press('/');
    assert.equal(await input.evaluate((element) => document.activeElement === element), true);
    await page.keyboard.type('keyboard');
    await page.keyboard.press('Tab');
    const tabFocus = await page.evaluate(() => {
      const active = document.activeElement;
      return {
        focused: active instanceof Element && active.classList.contains('search-button'),
        readyState: document.readyState,
        activeTag: active instanceof Element ? active.tagName.toLowerCase() : null,
        activeId: active instanceof Element ? active.id : null,
        activeClasses: active instanceof Element ? [...active.classList] : [],
      };
    });
    assert.equal(tabFocus.focused, true, `Tab should focus the search button: ${JSON.stringify(tabFocus)}`);
    await navigateBy(page, () => page.keyboard.press('Enter'));
    assert.equal(new URL(page.url()).searchParams.get('q'), 'keyboard');
    await page.locator('a.brand').focus();
    await keyboardAndMeasure(page, 'j');
    const activationLink = page.locator('.result-link').first();
    const resultFocus = await activationLink.evaluate((expected) => {
      const active = document.activeElement;
      return {
        focused: active === expected,
        readyState: document.readyState,
        activeTag: active instanceof Element ? active.tagName.toLowerCase() : null,
        activeId: active instanceof Element ? active.id : null,
        activeClasses: active instanceof Element ? [...active.classList] : [],
      };
    });
    assert.equal(resultFocus.focused, true, `j should focus the first result link: ${JSON.stringify(resultFocus)}`);
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
    const activation = await page.evaluate(() => window.__t03Activation);
    assert.equal(activation.href, linkEvidence.href);
    // Keyboard activation must not synthesize a pointer sequence. Gecko reports
    // click count 1 for Enter on a link; Chromium reports 0.
    assert([0, 1].includes(activation.detail), `unexpected activation detail ${activation.detail}`);
  });

  await check('viewport-narrow', async () => {
    await searchByQuery(page, 'valkey', { categories: 'general' });
    const explanation = page.locator('details.result-explanation');
    await explanation.getByText('Why this result appeared', { exact: true }).click();
    await page.setViewportSize({ width: 390, height: 844 });
    assert(await explanation.getByText('Cross-source presence').isVisible());
    assert(await explanation.getByText('structurally eligible', { exact: true }).isVisible());
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
    // A classic scrollbar can leave 305 CSS pixels available in a 320-pixel viewport.
    await page.setViewportSize({ width: 305, height: 640 });
    assert.equal(await viewportObservation(page, 'long-content-with-scrollbar-space'), true);
    await page.setViewportSize({ width: 320, height: 640 });
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
  if (throttled) await collectContractBudgets(landingMetric, resultsMetric, page);

  await context.close();
  contractBudgetProfileRequired = false;
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

async function runSourceAvailability() {
  const context = await newContext({ viewport: { width: 1440, height: 900 } });
  const page = await context.newPage();
  attachPageEvidence(page);
  await check('source-availability', async () => {
    const response = await searchByQuery(page, 'source-availability', { categories: 'general' });
    assert.equal(response.status(), 200);
    const notice = page.locator('.notice[role="status"]');
    const noticeText = await notice.innerText();
    assert(noticeText.includes('Google (blocked)'));
    assert(noticeText.includes('Reddit (rate limited)'));
    assert(noticeText.includes('Duckduckgo (successful scrape returned no results)'));
    const rail = page.locator('.results-rail');
    assert(await rail.getByText('Source status', { exact: true }).isVisible());
    for (const [name, status] of [
      ['Google', 'blocked'],
      ['Reddit', 'rate limited'],
    ]) {
      const row = rail
        .locator('.rail-source-unavailable')
        .filter({ has: page.getByText(name, { exact: true }) });
      assert.equal(await row.locator('.rail-source-name').innerText(), name);
      assert.equal(await row.locator('.rail-source-count').innerText(), status);
    }
    const emptyRow = rail
      .locator('.rail-source-empty')
      .filter({ has: page.getByText('Duckduckgo', { exact: true }) });
    assert.equal(await emptyRow.locator('.rail-source-name').innerText(), 'Duckduckgo');
    assert.equal(await emptyRow.locator('.rail-source-count').innerText(), 'no results');
    assert((await page.locator('body').innerText()).split('Google').length - 1 >= 2);
    const article = page.getByRole('article');
    assert.equal(await article.count(), 1);
    const title = 'A <script>alert("x")</script>';
    assert(await article.getByRole('heading', { name: title, exact: true }).isVisible());
    assert(await article.getByText('Research', { exact: true }).isVisible());
    const attribution = article.locator('.result-meta');
    assert(await attribution.getByText('Wikipedia', { exact: true }).isVisible());
    assert.equal(
      await article.getByRole('link', { name: title, exact: true }).getAttribute('href'),
      'https://example.com/result',
    );
    assert.equal(await page.locator('script').count(), 1);
    await screenshot(page, 'source-availability.png');

    const explanation = page.getByText(/^Specialists added by Jev:/);
    assert.equal(await explanation.count(), 0);
    const routed = await searchByQuery(page, 'jev-specialist-routing');
    assert.equal(routed.status(), 200);
    assert(await explanation.isVisible());
    assert.equal(await explanation.innerText(), 'Specialists added by Jev: PubMed');

    const scoped = await searchByQuery(page, 'jev-specialist-routing', { engines: 'wikipedia' });
    assert.equal(scoped.status(), 200);
    assert.equal(await explanation.count(), 0);

    // F:L615–F:L617 cover a successful empty scrape alongside a usable result.
    const emptyScrape = await searchByQuery(page, 'source-availability', {
      engines: 'duckduckgo,wikipedia',
    });
    assert.equal(emptyScrape.status(), 200);
    assert(await notice.getByText('No results from some sources.', { exact: true }).isVisible());
    assert((await notice.innerText()).includes('Duckduckgo (successful scrape returned no results)'));
    assert.equal(await page.getByText('Some sources could not answer this search.', { exact: true }).count(), 0);
    assert(await article.getByRole('heading', { name: title, exact: true }).isVisible());
  });
  await context.close();
}

async function writeRuntimeEvidence() {
  const path = pathFor(`${scenario}.browser-runtime-errors.json`);
  const document = {
    unexpected_requests: unexpectedRequests,
    synthetic_fixture_responses: syntheticFixtureResponses,
    console_errors: consoleErrors,
    http_error_responses: httpErrorResponses,
    http_error_console_statuses: httpErrorConsoleStatuses,
    engine_console_parity: engineConsoleParity,
    page_errors: pageErrors,
    request_failures: requestFailures,
    timing_control_evidence: interactionControls,
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

async function writeDateRoundtripEvidence() {
  if (scenario !== 'contract' || !dateRoundtripEvidence) return;
  const path = pathFor(scenario + '.browser-date-roundtrip.json');
  const bytes = Buffer.from(JSON.stringify(dateRoundtripEvidence, null, 2) + '\n');
  writeFileSync(path, bytes, { flag: 'wx' });
  artifacts.push({
    path,
    sha256: sha256(bytes),
    bytes: bytes.byteLength,
    kind: 'date-roundtrip',
    width: null,
    height: null,
  });
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

  browserInfo.playwright_version = playwrightPackage.metadata.version;
  const engine = process.env.T03_BROWSER_ENGINE || 'chromium';
  const engines = { chromium, firefox, webkit };
  assert(Object.hasOwn(engines, engine), `T03_BROWSER_ENGINE must be one of ${Object.keys(engines).join(', ')}`);
  browserInfo.name = engine;
  browserEngine = engine;
  engineConsoleParity = engine === 'chromium' || engine === 'webkit' ? 'chromium' : 'not-captured';
  browser = await engines[engine].launch({ headless: true });
  browserInfo.version = browser.version();

  if (scenario === 'contract') await runContract();
  else if (scenario === 'cached-error') await runErrorScenario('cached-error', 503, 'keep-cached');
  else if (scenario === 'rate-limited') await runErrorScenario('rate-limited', 429, 'keep-rate-limited');
  else if (scenario === 'html-disabled') await runErrorScenario('disabled-format', 403, 'keep-disabled-format');
  else if (scenario === 'unavailable') await runUnavailable();
  else if (scenario === 'source-availability') await runSourceAvailability();

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
    'source-availability': [],
  }[scenario];
  const observedHttpStatuses = httpErrorResponses.map((row) => row.status).sort((a, b) => a - b);
  const observedConsoleStatuses = [...httpErrorConsoleStatuses].sort((a, b) => a - b);
  const expectedSorted = [...expectedHttpErrors].sort((a, b) => a - b);
  if (JSON.stringify(observedHttpStatuses) !== JSON.stringify(expectedSorted)) {
    addFailure('http-status-inventory', `observed ${JSON.stringify(observedHttpStatuses)}; expected ${JSON.stringify(expectedSorted)}`);
  }
  if (engineConsoleParity === 'chromium' && JSON.stringify(observedConsoleStatuses) !== JSON.stringify(expectedSorted)) {
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

  await writeDateRoundtripEvidence();
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
    synthetic_fixture_responses: syntheticFixtureResponses,
    console_errors: consoleErrors,
    http_error_responses: httpErrorResponses,
    http_error_console_statuses: httpErrorConsoleStatuses,
    engine_console_parity: engineConsoleParity,
    search_summary_visible: searchSummaryVisible,
    page_errors: pageErrors,
    browser_closed: browserClosed,
    artifacts,
    budgets,
  };
  const receiptBytes = Buffer.from(`${JSON.stringify(receipt, null, 2)}\n`);
  writeFileSync(receiptPath, receiptBytes, { flag: 'wx' });
  process.stdout.write(`T03_BROWSER_COMPLETION ${JSON.stringify({ scenario, receipt: receiptPath, sha256: sha256(receiptBytes) })}\n`);
  if (failures.length > 0) {
    process.stderr.write(`T03_BROWSER_FAILURES ${JSON.stringify(failures)}\n`);
    process.exitCode = 1;
  }
}

main().catch(async (error) => {
  const timingControlEvidence = error?.timingControlsEvidence ?? error?.cause?.timingControlsEvidence;
  if (timingControlEvidence) interactionControls = timingControlEvidence;
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
        engine_console_parity: browserEngine === 'chromium' ? 'chromium' : 'not-captured',
        search_summary_visible: searchSummaryVisible,
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
