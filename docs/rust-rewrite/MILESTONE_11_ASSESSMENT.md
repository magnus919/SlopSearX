# Milestone 11 completion assessment

Measured 2026-11-05 against milestone 11 (`Rust Rewrite`) at `main` =
`efc70dec485cf47e82a59c54d19749ab3d32349a`.

**Milestone state: 23 open, 78 closed, still open.** This document records why
the remainder cannot be completed by autonomous work under the standing
rewrite authority, and the exact decision each open issue needs. It closes
nothing.

> Correction to an earlier reading in this session: #937 (G02) was briefly
> closed by a GitHub closing-keyword match in the merged PR #1101 body
> (`"does not close #937"` still matched the keyword `close` against `#937`).
> It has been reopened, and no G02 evidence was produced or withdrawn. This is
> why any earlier count of "22 open" undercounted by one.

## Method

- Milestone counters and each issue's native `blocked_by` edges were read from
  the GitHub API, not inferred from prose.
- Code maturity was measured in the checkout: file counts, line counts and the
  actual entry-point text of the affected binaries.
- Branch and pull-request state was read from the remote (`git rev-list`,
  `gh pr view`).

## Headline finding

The remaining issues are not a last mile. They are one unmerged foundation
(the native gateway), one unstarted foundation (the production search server),
and a set of qualification gates that need human-executed or human-authorized
evidence. No combination of automated changes can close any of the 23.

## Blocker classes

| class | meaning |
| --- | --- |
| **A — human authority / human evidence** | Requires a decision or evidence I am not authorised to produce (trust-store or authorization-policy change; manual visual and screen-reader review; real Safari/WebKit execution). |
| **B — sealed custody amendment** | Requires a new reviewed, SHA-sealed custody capsule before Python sources may be edited or retired. |
| **C — unmerged foundation branch** | The implementation exists only on a closed, unmerged branch. |
| **D — transitive** | Blocked by an issue in A, B or C through the native `blocked_by` graph. |
| **E — epic aggregator** | Closes only when its children close. |

## Per-issue classification

| issue | title | class | evidence |
| --- | --- | --- | --- |
| #1073 | macOS platform-trust qualification and cleanup | **A** | Requires a supported trust establishment *and* removal route. Its own text forbids protected-rule/SIP bypasses and says a genuinely new human authority is needed. The files it names (`crates/ssx-gateway/tests/platform_support/mac_trust.rs`, `.github/workflows/rust-client-platforms.yml`) do not exist on `main`. |
| #1075 | Windows platform-trust qualification and cleanup | **A** | Same shape: certificate-load boundary investigation plus a trust-policy decision. Its PS1 fixture and workflow also exist only off-`main`. |
| #902 | Ship native MCP gateway and OAuth client | **C** (A transitively) | `crates/ssx-gateway/src/main.rs` is an 11-line stub: `not implemented: native feature work is pending`, exit failure. The implementation is on `codex/preview-1056-t07-source-integration-20261027` — **110 commits ahead / 64 behind** `main`, last commit `884d50f2` on 2026-11-02. PR #1063 ("feat: ship native MCP gateway and OAuth client") was **closed unmerged** on 2026-11-02 with no review comments. #902 is blocked by #1073/#1075. |
| #903 | Replace `ssx` with a native Rust CLI | **D** | CLI is real (4,508 lines), but acceptance requires integration against "an actual separately launched native HTTP service". `crates/ssx-server/src/main.rs` is a *foundation readiness server*: `search is not implemented`, and it serves `ssx_infra::foundation_runtime()`, whose engine is `FakeEngine`. |
| #898 | Port public search portal and browser acceptance | **A** (automation complete) | The T03 gate already runs 15 journeys on Chromium and Firefox, including malicious content, keyboard-only, 320 CSS px, reduced motion, theme/storage failure and pagination; budgets are CDP-throttled separately from upstream latency. `docs/rust-rewrite/t03/CORE_DELIVERY.md` records the remaining acceptance as real Safari/WebKit, 200% browser zoom, manual visual review and screen-reader execution — all explicitly unavailable in the hosted runner. The browser client records `actual_browser_zoom_200_percent: false` rather than claiming it. |
| #919 | Replace Python health, deployment and upstream-contract tools | **B** | The native image-pin tools landed in #1102 and run in the scheduled promotion job. The remainder — wiring `image-pin-verify-base` into `docker.yml`, replacing `portal_release_smoke.py`, `configure_ci_dockerhub_mirror.py`, `upstream_contracts.py`, and deleting the Python twins — needs a new reviewed sealed `snapshot-fixture-amendment` capsule plus coordinated `python-disposition.json` / `python-invocations.json` updates. |
| #915 | Workflow MCP tools, prompts and registry parity | **D** | Blocked by #902. |
| #916 | Authenticated workflow console and browser mutations | **D** | Blocked by #898, #915. |
| #921 | Release artifacts, containers and install paths | **D** | Blocked by #902, #903, #919, #915. Also owns the production server composition seam (`HttpState::with_public_portal`), which #903 needs. |
| #934 | Replace backend-importing tests | **D** | Blocked by #915, #898, #902, #903, #916. |
| #936 | G01 integrated full-feature and all-adapter parity | **D** | Blocked by #916, #902, #903, #921, #934. |
| #937 | G02 security, recovery and overload | **D** | Blocked by #936. Reopened after an accidental auto-close; see the correction note above. |
| #938 | G03 resource and performance outcomes | **D** | Blocked by #936. |
| #939 | G04 durable-state migration and rollback | **D** | Blocked by #915. |
| #940 | G05 operator and contributor migration runbooks | **D** | Blocked by #915. |
| #941 | O07 backend-coupled acquisition drivers | **D** | Blocked by #940. |
| #942 | O09 required CI and release gates | **D** | Blocked by #934, #919, #941. |
| #943 | G06 isolated staging rehearsal | **D** | Blocked by #921, #939, #941, #942. |
| #944 | G07 canary and production cutover | **D** | Blocked by #936, #938, #943. Outside the standing authority in any case: production cutover is explicitly excluded. |
| #945 | G08 remove backend Python source and runtime | **D** | Blocked by #944. |
| #946 | O08 evidence receipts and qualification tooling | **D** | Blocked by #945, #934, #941, #942. |
| #947 | G09 final release verification and acceptance | **D** | Blocked by #946. |
| #879 | Epic: deliver the rewrite and retire the Python backend | **E** | Closes only when the above close. |

## Two structural problems worth an owner decision

1. **The critical path runs through a closed pull request.** #902's
   implementation is a 110-commit branch whose platform-trust qualification
   cannot pass on hosted runners. #1073/#1075 protect that qualification
   correctly — they refuse to weaken the witness — but they need authority that
   the routine rewrite mandate does not carry. Until someone decides how trust
   is established and removed in the qualification environment, or formally
   amends the T07 acceptance contract, the gateway cannot land and 17 of the 23
   open issues cannot start.

2. **#903 and #921 are mutually dependent in substance.** #921 is listed as
   blocked by #903, but the production server assembly that #903 needs lives in
   #921's scope. Neither can start first without an owner decision about which
   issue assembles `ssx-server` against the real engine registry and Valkey
   store instead of `foundation_runtime()`.

## What is genuinely complete

- **S01 / #1080** (closed): fail-closed local-fork advisory identity view,
  exact libxml provenance, deterministic real-checker fixtures, required CI
  wiring. PR #1101, `382d881f`.
- **O02 first increment / #919** (open): native `image-pin-promote` and
  `image-pin-verify-base` with verified registry parity, daily CI cross-check,
  and a fixed `bash -e` defect in digest promotion. PR #1102, `efc70dec`.
- **T03 automation / #898** (open): the portal and its 15-journey, dual-engine
  gate are real and green; only human-executed visual/assistive evidence and a
  real Safari/WebKit run remain.

## Decisions required

1. **#1073 / #1075** — authorise a supported trust establishment *and* removal
   route for the qualification environment, or amend the T07 platform contract
   to a scope the hosted runner can actually evidence.
   [T07 platform-trust proposal](../t07/PLATFORM_TRUST_PROPOSAL.md) sets out the
   four candidate routes, the exact commands, the authority each needs and a
   recommendation (system-domain trust under `sudo` on the ephemeral macOS
   runner, with a contract amendment as fallback). Nothing in it has been
   executed.
2. **#902** — decide whether the gateway branch lands once (1) is settled, and
   who owns the rebase of 110 commits across 64 commits of `main` drift.
3. **#903 / #921** — name the owner of production native server assembly and
   resolve the sequencing inversion. [ADR 008](../adr/008-native-runtime-roots-and-server-activation.md)
   proposes the contract, the fail-closed engine-assembly rule and the migration
   order, and assigns `ssx-infra` the assembler, `ssx-server` the activation,
   T08 the client integration and O04 the release composition.
4. **#919** — approve the sealed F01 amendment that retires the superseded
   Python sources.
5. **#898** — commit human-executed visual, screen-reader and real Safari/WebKit
   evidence, or amend `docs/PORTAL_ACCEPTANCE.md` to the automatable boundary.

None of these is a code defect that further autonomous work can resolve. This
assessment therefore stops short of claiming completion and leaves milestone 11
open.
