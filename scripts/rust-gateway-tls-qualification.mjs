import { execFileSync, spawn } from 'node:child_process';
import { createHash, randomBytes } from 'node:crypto';
import { once } from 'node:events';
import { createServer as createHttpServer } from 'node:http';
import { createServer as createHttpsServer } from 'node:https';
import { mkdtemp, readFile, realpath, rm, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { createInterface } from 'node:readline';
import { setTimeout as sleep } from 'node:timers/promises';

const protocol = '2025-11-25';
const maxRequestBytes = 16384;
const routePattern = /\|\s*(https:\/\/[a-z0-9-]+\.trycloudflare\.com)\s*\|/;

function require(condition, message) {
  if (!condition) throw new Error(message);
}

async function within(task, milliseconds, description) {
  const controller = new AbortController();
  try {
    return await Promise.race([
      task,
      sleep(milliseconds, undefined, { signal: controller.signal }).then(() => {
        throw new Error(description + ' timed out');
      }),
    ]);
  } finally {
    controller.abort();
  }
}

function start(executable, args, options) {
  const child = spawn(executable, args, options);
  const closed = once(child, 'close').then(([status]) => status);
  // A failed spawn can occur before the caller reaches its first protocol wait.
  closed.catch(() => {});
  return { child, closed };
}

async function stop({ child, closed }) {
  if (child.exitCode !== null || child.signalCode !== null || child.pid === undefined) return;
  child.kill();
  try {
    await within(closed, 3000, 'child shutdown');
  } catch {
    child.kill('SIGKILL');
    await within(closed, 3000, 'child termination');
  }
}

async function createCertificates(directory) {
  const root = join(directory, 'root.pem');
  const key = join(directory, 'root.key');
  const config = join(directory, 'root.cnf');
  const extension = join(directory, 'leaf.ext');
  const openssl = (...args) => execFileSync('openssl', args, { timeout: 15000, stdio: 'pipe' });
  await writeFile(config, [
    '[req]', 'distinguished_name=dn', 'x509_extensions=ca', '[dn]', '[ca]',
    'basicConstraints=critical,CA:TRUE', 'keyUsage=critical,keyCertSign,cRLSign',
    'subjectKeyIdentifier=hash', '',
  ].join('\n'));
  openssl(
    'req', '-x509', '-newkey', 'rsa:2048', '-nodes', '-sha256', '-config', config,
    '-subj', '/CN=Disposable SlopSearX test root', '-days', '2', '-keyout', key, '-out', root,
  );

  const leaves = [];
  for (const [index, name] of ['IP:127.0.0.1', 'DNS:invalid.example'].entries()) {
    const leafKey = join(directory, 'leaf-' + index + '.key');
    const request = join(directory, 'leaf-' + index + '.csr');
    const cert = join(directory, 'leaf-' + index + '.pem');
    await writeFile(extension, [
      'subjectAltName=' + name, 'basicConstraints=critical,CA:FALSE',
      'keyUsage=critical,digitalSignature,keyEncipherment', 'extendedKeyUsage=serverAuth', '',
    ].join('\n'));
    openssl(
      'req', '-new', '-newkey', 'rsa:2048', '-nodes',
      '-subj', '/CN=Disposable SlopSearX test leaf', '-keyout', leafKey, '-out', request,
    );
    openssl(
      'x509', '-req', '-sha256', '-in', request, '-CA', root, '-CAkey', key,
      '-set_serial', String(index + 1), '-days', '2', '-out', cert, '-extfile', extension,
    );
    openssl('verify', '-purpose', 'sslserver', '-CAfile', root, cert);
    leaves.push({ key: await readFile(leafKey), cert: await readFile(cert) });
  }
  return { root, leaves };
}

function respond(response, status, value) {
  if (value === undefined) {
    response.writeHead(status, { 'content-length': '0' }).end();
    return;
  }
  const body = JSON.stringify(value);
  response.writeHead(status, {
    'content-length': Buffer.byteLength(body),
    'content-type': 'application/json',
  }).end(body);
}

async function withFixture(certificate, task, { oauth = false } = {}) {
  const marker = randomBytes(16).toString('hex');
  const endpoint = '/' + marker + '/mcp';
  const requests = [];
  let origin;
  const results = {
    initialize: {
      protocolVersion: protocol,
      capabilities: { tools: {} },
      serverInfo: { name: 'tls-fixture', version: '1' },
    },
    'tools/list': { tools: [{ name: 'witness', inputSchema: { type: 'object' } }] },
    'tools/call': { content: [{ type: 'text', text: marker }] },
    'resources/list': { resources: [] },
    'resources/templates/list': { resourceTemplates: [] },
    'prompts/list': { prompts: [] },
  };

  function handleOAuth(request, response) {
    if (request.method === 'POST' && request.url === endpoint) {
      requests.push('mcp-challenge');
      const metadata = origin + '/.well-known/oauth-protected-resource' + endpoint;
      response.writeHead(401, {
        'www-authenticate': 'Bearer resource_metadata="' + metadata + '", scope="search"',
        'content-length': '0',
      }).end();
    } else if (request.method === 'GET' && request.url === '/.well-known/oauth-protected-resource' + endpoint) {
      requests.push('protected-resource');
      respond(response, 200, { resource: origin + endpoint, authorization_servers: [origin] });
    } else if (request.method === 'GET' && request.url === '/.well-known/oauth-authorization-server') {
      requests.push('authorization-server');
      respond(response, 200, {
        issuer: origin,
        authorization_endpoint: origin + '/authorize',
        token_endpoint: origin + '/token',
        registration_endpoint: origin + '/register',
        code_challenge_methods_supported: ['S256'],
        response_types_supported: ['code'],
      });
    } else if (request.method === 'POST' && request.url === '/register') {
      requests.push('registration');
      // End at the registration boundary, before client creation or browser authorization.
      respond(response, 400, { error: 'invalid_client_metadata' });
    } else {
      respond(response, 404);
    }
    request.resume();
  }

  async function handle(request, response) {
    if (oauth) {
      handleOAuth(request, response);
      return;
    }
    if (request.method === 'GET' && request.url === endpoint + '/ready') {
      respond(response, 200, marker);
      return;
    }
    if (request.method !== 'POST' || request.url !== endpoint) {
      respond(response, 404);
      return;
    }
    const buffers = [];
    let length = 0;
    for await (const chunk of request) {
      length += chunk.length;
      if (length > maxRequestBytes) {
        respond(response, 413);
        return;
      }
      buffers.push(chunk);
    }
    const message = JSON.parse(Buffer.concat(buffers));
    const method = message?.method;
    if (method === 'notifications/initialized') {
      requests.push(method);
      respond(response, 202);
    } else if (Object.hasOwn(results, method) && Object.hasOwn(message, 'id')) {
      requests.push(method);
      respond(response, 200, { jsonrpc: '2.0', id: message.id, result: results[method] });
    } else {
      respond(response, 400);
    }
  }

  const dispatch = (request, response) => {
    handle(request, response).catch(() => respond(response, 400));
  };
  const server = certificate ? createHttpsServer(certificate, dispatch) : createHttpServer(dispatch);
  server.requestTimeout = 10000;
  server.listen(0, '127.0.0.1');
  await once(server, 'listening');
  try {
    origin = (certificate ? 'https' : 'http') + '://127.0.0.1:' + server.address().port;
    return await task({ origin, endpoint, marker, requests });
  } finally {
    const closed = once(server, 'close');
    server.close();
    server.closeAllConnections();
    await within(closed, 3000, 'fixture shutdown');
  }
}

async function waitForPublicRoute(tunnel, fixture, log) {
  const deadline = Date.now() + 75000;
  let reason = 'no public hostname announced';
  let address;
  while (Date.now() < deadline && tunnel.child.exitCode === null && tunnel.child.signalCode === null) {
    address ??= log().match(routePattern)?.[1];
    if (address) {
      const endpoint = address + fixture.endpoint;
      try {
        const reply = await fetch(endpoint + '/ready', {
          redirect: 'error', signal: AbortSignal.timeout(3000),
        });
        if (reply.ok) {
          if (await reply.json() === fixture.marker) return { endpoint };
          reason = 'readiness response did not match this fixture';
        } else {
          reason = 'readiness returned HTTP ' + reply.status;
          await reply.body?.cancel();
        }
      } catch (error) {
        // Report error categories only. Keep URLs, fixture markers and tunnel credentials out of CI logs.
        const code = error.cause?.code ?? error.name;
        reason = 'readiness request failed (' + (/^[A-Za-z0-9_]+$/.test(code) ? code : 'unknown') + ')';
      }
    }
    await sleep(1000);
  }
  if (tunnel.child.exitCode !== null || tunnel.child.signalCode !== null) {
    reason += ', connector exited (' + (tunnel.child.exitCode ?? tunnel.child.signalCode) + ')';
  }
  return {
    reason,
    announced: Boolean(address),
    registered: log().includes('Registered tunnel connection'),
  };
}

async function withPublicRoute(cloudflared, directory, fixture, task) {
  // Try a new disposable route if readiness fails. Once ready, the gateway cases run exactly once.
  for (const protocol of ['auto', 'http2']) {
    const home = await mkdtemp(join(directory, 'tunnel-home-'));
    const tunnel = start(cloudflared, [
      'tunnel', '--no-autoupdate', '--protocol', protocol, '--url', fixture.origin,
    ], { cwd: home, env: { HOME: home, PATH: '' }, stdio: ['ignore', 'pipe', 'pipe'] });
    let log = '';
    for (const output of [tunnel.child.stdout, tunnel.child.stderr]) {
      output.setEncoding('utf8');
      output.on('data', (chunk) => { log = (log + chunk).slice(-65536); });
    }
    try {
      const result = await waitForPublicRoute(tunnel, fixture, () => log);
      if (result.endpoint) return await task(result.endpoint);
      console.error(
        'Temporary HTTPS route (' + protocol + ') unavailable. Hostname announced: '
        + result.announced + '. Connector registered: ' + result.registered + '. ' + result.reason,
      );
    } finally {
      await stop(tunnel);
    }
  }
  throw new Error('temporary HTTPS route did not become ready after two attempts');
}

async function exercise(binary, address, fixture, label, { bundle, reject, oauthState } = {}) {
  require(fixture.requests.length === 0, label + ': unexpected prior fixture request');
  const args = ['--remote', address];
  if (bundle) args.push('--ca-bundle', bundle);
  if (oauthState) {
    args.push('--oauth', '--oauth-no-browser', '--oauth-callback-port', '0', '--oauth-token-file', oauthState);
  }
  const env = { PATH: '' };
  for (const name of ['HOME', 'TMPDIR']) {
    if (process.env[name]) env[name] = process.env[name];
  }
  const running = start(binary, args, { env, stdio: ['pipe', 'pipe', 'pipe'] });
  const { child, closed } = running;
  let stderr = '';
  child.stderr.setEncoding('utf8');
  child.stderr.on('data', (data) => { stderr = (stderr + data).slice(-65536); });
  child.stdin.on('error', () => {}); // Early exits are reported by the protocol or exit checks.
  const lines = createInterface({ input: child.stdout })[Symbol.asyncIterator]();
  const send = (method, fields = {}) => {
    child.stdin.write(JSON.stringify({ jsonrpc: '2.0', method, ...fields }) + '\n');
  };
  const next = () => within(lines.next(), 20000, label + ': gateway reply');
  try {
    if (reject || oauthState) {
      const status = await within(closed, 20000, label + ': gateway exit');
      require(status === 1 && (await next()).done, label + ': expected a clean rejection');
      require(stderr.includes('Could not connect to the remote MCP server'), label + ': connection error missing');
      const requests = fixture.requests.splice(0);
      if (oauthState && bundle) {
        const expected = ['mcp-challenge', 'protected-resource', 'authorization-server', 'registration'];
        let offset = 0;
        for (const request of requests) {
          if (request === expected[offset]) offset++;
        }
        require(offset === expected.length, label + ': did not reach the registration boundary');
      } else {
        require(requests.length === 0, label + ': request reached HTTP');
      }
      return { case: label, exit: status, http_requests: requests };
    }
    send('initialize', {
      id: 1,
      params: {
        protocolVersion: protocol, capabilities: {},
        clientInfo: { name: 'tls-qualification', version: '1' },
      },
    });
    const initialize = JSON.parse((await next()).value);
    require(initialize.id === 1 && initialize.result?.protocolVersion === protocol, label + ': invalid initialization');
    send('notifications/initialized');
    send('tools/call', { id: 2, params: { name: 'witness', arguments: {} } });
    const call = JSON.parse((await next()).value);
    require(
      call.id === 2 && call.result?.content?.some(({ type, text }) => type === 'text' && text === fixture.marker),
      label + ': fixture response missing',
    );
    child.stdin.end();
    const status = await within(closed, 8000, label + ': gateway shutdown');
    require(status === 0, label + ': gateway exited unsuccessfully');
    const requests = fixture.requests.splice(0);
    require(requests.includes('initialize') && requests.includes('tools/call'), label + ': fixture request missing');
    return { case: label, exit: status, http_requests: requests };
  } finally {
    await stop(running);
  }
}

async function main() {
  const [binaryArg, tunnelArg, runRoot, output] = process.argv.slice(2);
  const revision = process.env.GITHUB_SHA;
  require(revision && binaryArg && tunnelArg && runRoot && output, 'missing source revision or fixture paths');
  const binary = await realpath(binaryArg);
  const cloudflared = await realpath(tunnelArg);
  const digest = async (path) => createHash('sha256').update(await readFile(path)).digest('hex');
  const script = new URL(import.meta.url);
  const installedDigest = await digest(binary);
  const fixtureDigest = await digest(script);
  const directory = await mkdtemp(join(runRoot, 't07-tls-'));
  const cases = [];
  const oauthDiscovery = [];
  let publicHostname;
  try {
    const { root, leaves: [valid, wrong] } = await createCertificates(directory);
    await withFixture(valid, async (fixture) => {
      const address = fixture.origin + fixture.endpoint;
      cases.push(await exercise(binary, address, fixture, 'untrusted-chain', { reject: true }));
      cases.push(await exercise(binary, address, fixture, 'private-with-bundle', { bundle: root }));
    });
    await withFixture(wrong, async (fixture) => {
      cases.push(await exercise(binary, fixture.origin + fixture.endpoint, fixture, 'wrong-hostname', {
        bundle: root, reject: true,
      }));
    });
    await withFixture(valid, async (fixture) => {
      const address = fixture.origin + fixture.endpoint;
      oauthDiscovery.push(await exercise(binary, address, fixture, 'oauth-untrusted-chain', {
        oauthState: join(directory, 'oauth-default.json'), reject: true,
      }));
      oauthDiscovery.push(await exercise(binary, address, fixture, 'oauth-discovery-with-bundle', {
        oauthState: join(directory, 'oauth-with-bundle.json'), bundle: root,
      }));
    }, { oauth: true });
    await withFixture(null, (fixture) => withPublicRoute(cloudflared, directory, fixture, async (address) => {
      publicHostname = new URL(address).hostname;
      cases.push(await exercise(binary, address, fixture, 'public-default-trust'));
      cases.push(await exercise(binary, address, fixture, 'public-with-private-bundle', { bundle: root }));
    }));
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
  require(await digest(binary) === installedDigest, 'gateway changed during qualification');
  require(await digest(script) === fixtureDigest, 'fixture changed during qualification');
  await writeFile(output, JSON.stringify({
    schema_version: 1,
    revision,
    run_id: process.env.GITHUB_RUN_ID,
    run_attempt: process.env.GITHUB_RUN_ATTEMPT,
    runner_arch: process.arch,
    runner_os: process.platform,
    installed_gateway_sha256: installedDigest,
    fixture_sha256: fixtureDigest,
    public_hostname: publicHostname,
    cases,
    oauth_discovery: oauthDiscovery,
    fixtures_retired: true,
  }, null, 2) + '\n');
}

await main();
