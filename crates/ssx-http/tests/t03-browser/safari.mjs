import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { createReadStream, createWriteStream } from 'node:fs';
import { mkdir, writeFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { createServer, Socket } from 'node:net';
import { join } from 'node:path';
import { createInterface } from 'node:readline';
import { setTimeout as delay } from 'node:timers/promises';

const [fixturePath, evidence] = process.argv.slice(2);
assert(fixturePath && evidence, 'usage: node safari.mjs FIXTURE EVIDENCE_DIR');
await mkdir(evidence, { recursive: true });
const receipt = { revision: process.env.GITHUB_SHA, cases: [] };

function start(command, args, name) {
  const child = spawn(command, args);
  const exited = once(child, 'exit');
  child.stderr.pipe(createWriteStream(join(evidence, name + '.stderr')));
  return { child, exited };
}

async function retire({ child, exited }, graceful) {
  const hardStop = setTimeout(() => child.kill('SIGKILL'), 8000);
  try {
    if (child.exitCode === null && child.signalCode === null) {
      if (graceful) child.stdin.end('STOP\n');
      else child.kill('SIGTERM');
    }
    return await exited;
  } finally {
    clearTimeout(hardStop);
  }
}

async function loopbackPort() {
  const socket = createServer().listen(0, '127.0.0.1');
  await once(socket, 'listening');
  const { port } = socket.address();
  await new Promise((resolve, reject) => socket.close(error => error ? reject(error) : resolve()));
  return port;
}

async function stopped(port) {
  const socket = new Socket();
  try {
    await new Promise((resolve, reject) => {
      socket.once('connect', () => reject(new Error('loopback port still open: ' + port)));
      socket.once('error', error => error.code === 'ECONNREFUSED' ? resolve() : reject(error));
      socket.setTimeout(1000, () => reject(new Error('loopback close check timed out')));
      socket.connect(port, '127.0.0.1');
    });
  } finally {
    socket.destroy();
  }
}

async function command(origin, path, method = 'GET', body) {
  const response = await fetch(origin + path, {
    method,
    headers: { 'content-type': 'application/json' },
    body: body === undefined ? undefined : JSON.stringify(body),
    signal: AbortSignal.timeout(20_000),
  });
  const result = await response.json();
  assert(response.ok, method + ' ' + path + ': ' + JSON.stringify(result.value));
  return result.value;
}

const fixture = start(fixturePath, [join(evidence, 'adapter-calls.jsonl'), 'dark', 'contract'], 'fixture');
let fixtureUrl;
let driver;
let driverPort;
let session;
try {
  const lines = createInterface({ input: fixture.child.stdout });
  let readyLine;
  try {
    [readyLine] = await once(lines, 'line', { signal: AbortSignal.timeout(15_000) });
  } finally {
    lines.close();
    fixture.child.stdout.resume();
  }
  const ready = JSON.parse(readyLine);
  assert.match(ready.address, /^127\.0\.0\.1:[1-9][0-9]{0,4}$/);
  assert.equal(ready.mode, 'offline-t03-fixture');
  assert.equal(ready.renderer, 'PublicPortal');
  assert.equal(ready.scenario, 'contract');
  fixtureUrl = new URL('http://' + ready.address);
  receipt.fixture = ready;
  const binary = createHash('sha256');
  for await (const chunk of createReadStream(fixturePath)) binary.update(chunk);
  receipt.fixture_sha256 = binary.digest('hex');

  driverPort = await loopbackPort();
  const driverUrl = 'http://127.0.0.1:' + driverPort;
  driver = start('/usr/bin/safaridriver', ['--port', String(driverPort)], 'safaridriver');
  driver.child.stdout.resume();
  let available = false;
  for (let attempt = 0; attempt < 30 && !available; attempt += 1) {
    try {
      available = (await command(driverUrl, '/status')).ready;
    } catch (error) {
      if (error.cause?.code !== 'ECONNREFUSED') throw error;
    }
    if (!available) await delay(200);
  }
  assert(available, 'native Safari driver did not become ready');
  const opened = await command(driverUrl, '/session', 'POST', {
    capabilities: { alwaysMatch: { browserName: 'safari' } },
  });
  session = '/session/' + opened.sessionId;
  const browser = opened.capabilities;
  assert.equal(browser.browserName.toLowerCase(), 'safari');
  receipt.browser = { name: browser.browserName, version: browser.browserVersion };
  const call = (path, method, body) => command(driverUrl, session + path, method, body);
  const navigate = path => call('/url', 'POST', { url: new URL(path, fixtureUrl).href });
  const execute = script => call('/execute/sync', 'POST', { script, args: [] });
  const element = selector => call('/element', 'POST', {
    using: 'css selector', value: selector,
  }).then(value => value['element-6066-11e4-a52e-4f735466cecf']);
  const screenshot = async name => writeFile(
    join(evidence, name + '.png'), Buffer.from(await call('/screenshot'), 'base64'),
  );

  async function resultsPage(query, page) {
    let observation;
    for (let attempt = 0; attempt < 30; attempt += 1) {
      observation = await execute(`
        return {
          href: location.href,
          title: document.querySelector('#results-title')?.textContent.trim(),
          page: document.querySelector('[aria-current="page"]')?.textContent.trim(),
        };
      `);
      const url = new URL(observation.href);
      if (url.origin === fixtureUrl.origin && url.pathname === '/search'
          && url.searchParams.get('q') === query && observation.title === query
          && observation.page?.split(' ')[1] === String(page)) return url;
      await delay(200);
    }
    assert.fail('Safari did not render the expected results page: ' + JSON.stringify(observation));
  }

  await navigate('/');
  const query = await element('#portal-query');
  assert.equal(await execute('return document.activeElement.id'), 'portal-query');
  await call('/element/' + query + '/value', 'POST', { text: 'keyboard\uE007' });
  await resultsPage('keyboard', 1);
  await screenshot('keyboard-search');
  receipt.cases.push('keyboard-search');

  await navigate('/search?q=valkey&categories=general');
  await resultsPage('valkey', 1);
  const next = await element('a[aria-label="Try next result page"]');
  await call('/element/' + next + '/click', 'POST', {});
  const pageTwo = await resultsPage('valkey', 2);
  assert.equal(pageTwo.searchParams.get('categories'), 'general');
  assert.equal(pageTwo.searchParams.get('pageno'), '2');
  await element('a[aria-label="Previous result page"]');
  await element('a[aria-label="Try next result page"]');
  await screenshot('pagination-page-two');
  await call('/back', 'POST', {});
  assert.equal((await resultsPage('valkey', 1)).searchParams.get('pageno'), null);
  await call('/forward', 'POST', {});
  assert.equal((await resultsPage('valkey', 2)).searchParams.get('pageno'), '2');
  receipt.cases.push('pagination-history');
} catch (error) {
  receipt.error = error.message;
  throw error;
} finally {
  try {
    if (session) {
      await command('http://127.0.0.1:' + driverPort, session, 'DELETE');
      receipt.session_closed = true;
    }
  } finally {
    try {
      if (driver) {
        await retire(driver, false);
        await stopped(driverPort);
        receipt.driver_port_closed = true;
      }
    } finally {
      try {
        const [code, signal] = await retire(fixture, true);
        receipt.fixture_exit = { code, signal };
        assert.equal(code, 0, 'fixture did not stop gracefully');
        if (fixtureUrl) {
          await stopped(Number(fixtureUrl.port));
          receipt.fixture_port_closed = true;
        }
      } finally {
        await writeFile(join(evidence, 'safari.json'), JSON.stringify(receipt, null, 2) + '\n');
      }
    }
  }
}
