# F04 replay corpus

`crates/xtask/fixtures/replay/corpus.json` contains five portable observations: npm, crates.io and OpenAlex adapter exchanges; a SearXNG JSON search response; and an MCP search response with structured filter enforcement. The three adapter cases replay exact request order and sanitized HTTP responses through a loopback-only server. The two surface cases execute the pinned application and MCP harness with deterministic fake engines and an in-memory store. All five expected/candidate envelopes were captured from two independent Python source executions. They are source observations, not Rust parity results.

The surface case IDs are `search-http-json-contract` and `mcp-search-filter-enforcement`. The MCP case requests `language=de`, `time_range=week`, and `safesearch=moderate`; its complete enforcement report is retained and currently reports `unsupported` for each filter because the fake engine enforces none. The search HTTP case retains the complete SearXNG JSON result and metadata schema. The adapter results use the complete `SearchResult` dataclass shape and sorted engine provenance.

The corpus records baseline Git blob IDs for all Python adapters, fixtures, application surfaces and source tests. `provenance.oracle` records the developer exporter path and SHA-256. The only normalized values are measured latency and response time, replaced with numeric zero during export to make the corpus byte-stable. The Rust comparator still validates their numeric types and non-negative values. Status, schema, enforcement, provenance, counts, result order, null/omission, request details and all other values remain exact.

The response bodies use the existing reviewed upstream fixtures at `tests/fixtures/upstream_contracts/{npm,crates,openalex}.json`. Only their allowlisted public parser fields are retained; no identity headers, credentials, author/publisher identities, raw errors or unfiltered payloads are included. Corpus structure and generated observations are MIT-licensed with this repository. The embedded sanitized upstream API facts retain their source terms; this corpus does not claim an additional license grant.

## Export and replay

The exporter runs pinned Python source from a clean detached checkout and writes output into the current checkout. From the repository root:

```sh
git worktree add --detach /private/tmp/slopsearx-f04-oracle-src 7b2d876307189d7a62000d1f289b3ef4b65eaff6
.venv/bin/python docs/rust-rewrite/replay/oracle/export.py --source-root /private/tmp/slopsearx-f04-oracle-src --baseline 7b2d876307189d7a62000d1f289b3ef4b65eaff6
cargo xtask check F04
```

The exporter rejects a dirty or wrong-revision source checkout, including untracked files and ignored Python bytecode under the pinned source packages. It starts from an environment allowlist (process essentials and explicit replay controls only), uses a private temporary home, fixes the HTTP surface to JSON, and sets `trust_env=False` on its HTTP clients. The HTTP observation keeps the real FastAPI lifespan and request/service pipeline, while injecting the pinned contract engine and the same `InMemoryStore` used by the test harness. It seeds a deterministic in-memory config snapshot and traps any attempt to read the host `/etc/slopsearx/config.yaml`. The MCP observation uses the existing fixture server and in-memory state. The exporter permits only its explicitly registered loopback fixture ports, rejects Unix/datagram sockets, blocks DNS, and uses only sanitized recorded responses. It does not call live or paid providers. It verifies each fixture source blob against the recorded baseline before writing the corpus.

In `--candidate` mode, the helper sets `HOME=/dev/null` and leaves XDG config/cache variables unset. The adapter only needs its explicit replay inputs and loopback URL; it writes no home or cache data. This allows `replay-run` to keep the candidate root read-only.

To verify format-setting isolation without overwriting the corpus, run:

```sh
.venv/bin/python docs/rust-rewrite/replay/oracle/export.py \
  --source-root /private/tmp/slopsearx-f04-oracle-src \
  --baseline 7b2d876307189d7a62000d1f289b3ef4b65eaff6 \
  --verify-ambient-format
```

This produces clean-environment and poisoned-environment exports in temporary
files, then compares their complete bytes. The poisoned run sets `SEARCH_FORMATS=html`,
hostile Valkey URLs, Sentry and provider credentials, workflow-portal activation,
and runtime/routing/feature/payload limits. A local Valkey sentinel has a
positive-control connection before the exports, then verifies that neither
export connects to it. The exporter also traps host config reads, and the
allowlisted environment removes all ambient runtime settings before importing
the pinned app. Outputs are compared as complete bytes, not hashes or selected
fields.

`cargo xtask check F04` verifies source blob provenance, the deterministic input/clock/ID fields, the exact transcript and request count/order, envelope schemas and the full expected/candidate comparison. The MCP check requires the `enforcement` object and its field-level statuses. The check must fail for changed status, schema, provenance, request count/order, filter enforcement, result order, omitted/null changes, or non-volatile values.

When a native candidate exists, run it against one case with:

```sh
cargo xtask replay-run crates/xtask/fixtures/replay/corpus.json mcp-search-filter-enforcement -- ./path/to/native-candidate
```

`replay-run` supplies `SSX_REPLAY_URL`, `SSX_REPLAY_ORIGIN`, `SSX_FIXED_UNIX_SECONDS`, `SSX_FIXED_IDS`, and `SSX_REPLAY_INPUT`. The candidate must print one `{"status": ..., "body": ...}` observation to stdout; stdout is parsed and compared to the pinned Python `expected` envelope. Its HTTP requests are served by the local transcript and checked for origin, method, target, headers, body, count and order. The current workspace has no completed native search or MCP candidate, so this command is an interface for later ports and currently demonstrates no Rust parity.

The exporter command is optional developer tooling. Python is never invoked by Rust runtime, native replay, mandatory native CI/release gates, deployment, scheduled jobs or active backend operation. The corpus provides source observations to compare against native implementations; it does not certify that any Rust adapter or surface is at parity until native candidate outputs are supplied and the differential gate passes.

## Refresh procedure

1. Start with a clean detached worktree at the recorded Python baseline. The exporter requires the source commit and rejects tracked modifications.
2. Do not update observations in response to a failure. First determine whether the source, fixture, or export path changed. Any accepted behavior change belongs in the Rust rewrite parity ledger.
3. For upstream cases, use only the already-reviewed sanitized fixtures unless a separately reviewed bounded capture is authorized. Never include auth, cookies, personal fields, proxy headers or raw provider exceptions.
4. Preserve exact methods, relative targets and query strings, response status/headers/body, full result order, complete output fields and provenance. Add only a specifically justified numeric volatility pointer from the comparator allowlist.
5. Inspect the generated corpus diff and source blob list before merging. The fixed exporter command must reproduce the corpus byte-for-byte at the same source and exporter revisions.

## Temporary Python oracle boundary

The exporter imports the historical backend only in a developer process. It consumes fixture outputs and writes JSON; it does not bridge Python into Rust or provide a fallback runtime. Rust adapter and surface work must retain native tests and compare their actual outputs against this corpus. Once all consumers use accepted native evidence, Python oracle invocation can be retired while keeping the historical fixture bytes and their provenance. This path adds one developer-only Python file, `docs/rust-rewrite/replay/oracle/export.py`, for inclusion in the rewrite's Python invocation/source inventory.

## OS-enforced offline candidates

`replay-run` requires OS network confinement before starting any candidate; setting
`SSX_REPLAY_URL` alone is insufficient. The native F04 gate exercises both the
actual confined runner with a native plumbing canary and an isolated socket
probe: loopback replay must succeed, while a connection to the RFC 5737
reserved documentation address `203.0.113.1:9` must fail with an OS permission
or isolated-network routing error. Timeouts and refused connections are not
accepted as proof of confinement. The canary deliberately echoes a fixture
observation and establishes containment/transcript behavior only.

On macOS, `/usr/bin/sandbox-exec` applies a mandatory inherited Seatbelt policy
that denies network operations except inbound/outbound loopback IP sockets.
The command fails before the candidate if this platform facility is absent or
rejected. Its deprecated status does not authorize an unrestricted fallback.

On Linux, install `bubblewrap` at `/usr/bin/bwrap`. The complete runner and
fixture server execute together in a private user/network/PID namespace with
only loopback networking, no effective capabilities, and a read-only root
mount. Bubblewrap brings loopback up before dropping capabilities. Before
starting the candidate, the inner runner verifies that its network namespace
differs from the outer one, `/proc/net/dev` contains only `lo`, effective
capabilities are zero, and the real loopback/egress probes pass. A command-line
marker alone cannot bypass these checks. Nested user namespaces are disabled.

The Rust CI installs bubblewrap on both Linux matrix targets. Hosts must permit
its user namespaces through their kernel and LSM policy. If an AppArmor policy
blocks the packaged binary, use a reviewed executable-specific policy for
`/usr/bin/bwrap`; do not disable host protections globally. Namespace/LSM
rejection, missing tools, and unsupported platforms stop execution before the
candidate. There is no proxy-only, environment-only, or unrestricted fallback.

### Ubuntu CI user-namespace policy

Run `38802073443` failed on both Ubuntu 24.04 architectures before candidate
execution: bubblewrap `0.9.0-1ubuntu0.3` could not configure loopback
(`RTM_NEWADDR: Operation not permitted`). This identifies a namespace setup
permission failure; the historical logs do not establish which installed
AppArmor profile caused it. The next CI setup records existing attachments,
the same bubblewrap command before and after loading the targeted policy, and
available kernel-denial diagnostics. Unexpected attachment identities fail.

Ubuntu's [user-namespace restriction guidance](https://discourse.ubuntu.com/t/understanding-apparmor-user-namespace-restriction/58007)
recommends a purpose-built bubblewrap profile rather than a broad unconfined
user-namespace exemption. The inline CI policy follows the setup/stacked-child
pattern in [AppArmor's bwrap-userns-restrict policy](https://gitlab.com/apparmor/apparmor/-/raw/master/profiles/apparmor/profiles/extras/bwrap-userns-restrict)
(retrieved 2026-10-15; source SHA-256
`d61facde27707b9c47ffe47921b7273e788784484cb5530eb819e6daac1f1990`),
using Noble's ABI 4.0 and the existing `bwrap`
executable-attachment identity. Setup can use capabilities needed for namespace
and loopback creation; the child profile explicitly denies every capability.
The runner additionally keeps `--cap-drop ALL`, verifies `CapEff` is zero,
and disables further user namespaces. No global AppArmor or user-namespace
sysctl is weakened.

The released [bubblewrap 0.9.0 setup source](https://github.com/containers/bubblewrap/blob/v0.9.0/bubblewrap.c)
retains required setup capabilities for `loopback_setup` and drops privileges
before executing the program. Removing the runner's capability drop would
therefore be an unsupported response to the observed failure. Successful
before/after CI probes and the mandatory native isolation cases must establish
the corrected runtime behavior; installing a profile alone is not acceptance.
