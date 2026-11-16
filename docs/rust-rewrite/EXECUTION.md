# Rust Rewrite execution and acceptance

The maintainer authorized execution of the complete Rust Rewrite milestone on
2026-10-15. The target remains the full [case-study contract](CASE_STUDY.md),
including native backend, tooling, gateway and ssx, operational qualification,
cutover, retirement and final acceptance. No intermediate foundation is a
substitute for milestone completion.

## Execution ownership

- Orchestrator: parent agent, accountable for integration, review, acceptance,
  issue evidence, dependency readiness and final completion audit.
- General execution: GPT-6 Luna (`gpt-6-luna`).
- Particularly complex architecture, identity and distributed-state work:
  GPT-6.1 Sol (`gpt-6.1-sol`), per the maintainer update on 2026-10-15.
  Earlier F01/F02 assignments used GPT-6 Sol; new complex assignments use 6.1.
- Use isolated task worktrees. Within a task, parallel agents own disjoint files;
  the orchestrator owns integration and shared contracts.

An issue starts only after its hard prerequisites have accepted and merged
handoffs. Open status alone does not mean ready. No issue closes merely because
an agent reports completion. Inspect actual source, run relevant checks, inspect
review findings and exact-head CI, and verify the merged artifact before closure.

## Execution ledger

| Plan | Issue | State | Evidence / next gate |
| --- | --- | --- | --- |
| F01 | [#880](https://github.com/magnus919/SlopSearX/issues/880) | Accepted | [PR #950](https://github.com/magnus919/SlopSearX/pull/950), merged `2e0026b4e471c9309a898665169578333074015e`; [inventory evidence](inventory/validation.json), exact-head CI passed, independent Sol corrections and Luna final audit accepted |
| F02 | [#881](https://github.com/magnus919/SlopSearX/issues/881) | Accepted | [PR #952](https://github.com/magnus919/SlopSearX/pull/952), merged `562b6ce1ce9f798104311e8f3a213daefeddc795`; ADR 003–005 and state/pin registers; corrected membership classification, positive Droid re-review and exact-head CI passed |
| F03 | [#882](https://github.com/magnus919/SlopSearX/issues/882) | Accepted | [PR #955](https://github.com/magnus919/SlopSearX/pull/955), merged `7b2d876307189d7a62000d1f289b3ef4b65eaff6`; exact-head CI and completed Droid re-review; [accepted handoff](https://github.com/magnus919/SlopSearX/issues/882#issuecomment-6200997449), retained Linux amd64/arm64 native evidence |
| F04 | [#883](https://github.com/magnus919/SlopSearX/issues/883) | Accepted | [PR #957](https://github.com/magnus919/SlopSearX/pull/957), merged `16f03be656f34d90edd8285f9c81dfb1c4626b0a`; exact-head CI, independent checks and final review passed; [accepted handoff](https://github.com/magnus919/SlopSearX/issues/883#issuecomment-6202578924); retained Linux amd64/arm64 replay evidence |
| F05 | [#884](https://github.com/magnus919/SlopSearX/issues/884) | Accepted | [PR #960](https://github.com/magnus919/SlopSearX/pull/960), merged `7cc420f7ffb61127750e77bf6130e225925f9186`; typed domain and stable serialization evidence; [accepted handoff](https://github.com/magnus919/SlopSearX/issues/884#issuecomment-6203899560) |
| F06 | [#885](https://github.com/magnus919/SlopSearX/issues/885) | Accepted | [PR #967](https://github.com/magnus919/SlopSearX/pull/967), merged `1f3f5bc85083f4fd24bd9a2b07f161626e6bfef5`; [accepted handoff](https://github.com/magnus919/SlopSearX/issues/885#issuecomment-6208497732), typed config and complete engine catalog |
| F07 | [#886](https://github.com/magnus919/SlopSearX/issues/886) | Accepted | [PR #961](https://github.com/magnus919/SlopSearX/pull/961), merged `cfd7a0af61a22cfd040a7fcd41ad950f82005959`; native Valkey lifecycle and exact atomic-script evidence; [accepted handoff](https://github.com/magnus919/SlopSearX/issues/886#issuecomment-6204292044), [scope and parity ledger](f07/README.md) |
| F08 | [#887](https://github.com/magnus919/SlopSearX/issues/887) | Accepted | [PR #962](https://github.com/magnus919/SlopSearX/pull/962), merged `9ab1c7a22de933762c42469145c1f50365220de2`; [accepted handoff](https://github.com/magnus919/SlopSearX/issues/887#issuecomment-6205234733), independent source review and exact-head Linux amd64/arm64 CI passed |
| F09 | [#888](https://github.com/magnus919/SlopSearX/issues/888) | Accepted | [PR #970](https://github.com/magnus919/SlopSearX/pull/970), merged `6ae5e34fed19e8e9da7c19b95eeea894512f0e6d`; [accepted handoff](https://github.com/magnus919/SlopSearX/issues/888#issuecomment-6211563513), independent full review and exact-head CI; external review pending, not approval |
| F10 | [#889](https://github.com/magnus919/SlopSearX/issues/889) | Accepted | [PR #966](https://github.com/magnus919/SlopSearX/pull/966), merged `c326a3d5a75fd299b05ebe366eac816352b3d36d`; native quota authority, provider cooldown and observed-health evidence; [accepted handoff](https://github.com/magnus919/SlopSearX/issues/889#issuecomment-6206445353), [scope/evidence](f10/README.md) |
| F11 | [#890](https://github.com/magnus919/SlopSearX/issues/890) | Accepted | [PR #963](https://github.com/magnus919/SlopSearX/pull/963), merged `fd80e36e6e1868d5c47f3f3d93c78952fcbc20e5`; [accepted handoff](https://github.com/magnus919/SlopSearX/issues/890#issuecomment-6204838740), independently recaptured source corpus and exact-head Linux amd64/arm64 CI passed |
| F12 | [#891](https://github.com/magnus919/SlopSearX/issues/891) | Accepted | [PR #972](https://github.com/magnus919/SlopSearX/pull/972), merged `d8bcf5ede0cfa0b359f8bd25cdca89c23366b7e2`; [accepted handoff](https://github.com/magnus919/SlopSearX/issues/891#issuecomment-6213032913), native optional routing and exact-head qualification |
| F13 | [#892](https://github.com/magnus919/SlopSearX/issues/892) | Accepted | [PR #976](https://github.com/magnus919/SlopSearX/pull/976), merged `730c37a5f48b9830088594582cefb4c2f10a35e8`; [accepted handoff](https://github.com/magnus919/SlopSearX/issues/892#issuecomment-6215395092), canonical cache and immutable snapshot evidence |
| F14 | [#893](https://github.com/magnus919/SlopSearX/issues/893) | Accepted | [PR #978](https://github.com/magnus919/SlopSearX/pull/978), merged `576d43b03c0378ffe113a1386b1ec88eaa4323df`; [accepted handoff](https://github.com/magnus919/SlopSearX/issues/893#issuecomment-6218080080), shared execution and exact-head Linux amd64/arm64 qualification |
| F15 | [#894](https://github.com/magnus919/SlopSearX/issues/894) | Implementation in progress; acceptance pending | Native bounded reranker and read-boundary advisories; [scope and boundaries](f15/README.md). Full independent package review, final qualification and exact-head publication evidence remain required |
| A01 | [#917](https://github.com/magnus919/SlopSearX/issues/917) | Accepted | [PR #969](https://github.com/magnus919/SlopSearX/pull/969), merged `025ab2f2d50febcb573ba62579eecf7e2d07ef3a`; [accepted handoff](https://github.com/magnus919/SlopSearX/issues/917#issuecomment-6212053625), native npm/crates adapters and exact source qualification |
| A02 | [#922](https://github.com/magnus919/SlopSearX/issues/922) | Accepted | [PR #974](https://github.com/magnus919/SlopSearX/pull/974), merged `01876639fb8d87123f12293c02821981cff3f8b9`; [accepted handoff](https://github.com/magnus919/SlopSearX/issues/922#issuecomment-6214248398), independent full review and exact-head CI passed |
| A03 | [#923](https://github.com/magnus919/SlopSearX/issues/923) | Accepted | [PR #975](https://github.com/magnus919/SlopSearX/pull/975), merged `5c1b48b9e625501ab61828299168101ac00301b3`; independent full review and exact-head Linux amd64/arm64 qualification; [scope and parity](a03/README.md) |
| T01 | [#895](https://github.com/magnus919/SlopSearX/issues/895) | Accepted | [PR #959](https://github.com/magnus919/SlopSearX/pull/959), merged `5058056b02c5ebe523e61acfaadf19795f664be0`; native MCP SDK, transport and lifecycle qualification; [accepted handoff](https://github.com/magnus919/SlopSearX/issues/895#issuecomment-6206106156) |
| T04 | [#899](https://github.com/magnus919/SlopSearX/issues/899) | Accepted | [PR #968](https://github.com/magnus919/SlopSearX/pull/968), merged `403dfd3720e969fa829f426136bf0f05b0360c56`; [accepted handoff](https://github.com/magnus919/SlopSearX/issues/899#issuecomment-6210638511), full independent review and exact-head Linux amd64/arm64 CI passed; external review provider failed and is not approval |
| T05 | [#900](https://github.com/magnus919/SlopSearX/issues/900) | Accepted | [PR #973](https://github.com/magnus919/SlopSearX/pull/973), merged `66b29c3f8437af45a4f0d28f7c54a0f0727f7fa5`; [accepted handoff](https://github.com/magnus919/SlopSearX/issues/900#issuecomment-6213768672), independently reviewed native OAuth endpoints and exact-head qualification; T06 owns production composition |
| T09 | [#904](https://github.com/magnus919/SlopSearX/issues/904) | Accepted | [PR #971](https://github.com/magnus919/SlopSearX/pull/971), merged `35d97ccf8c9b2d8246373b2024cc8d7a457705a3`; [accepted handoff](https://github.com/magnus919/SlopSearX/issues/904#issuecomment-6212567586), exact reviewed tree `fc81abc064015930ec136758ccfc551ac1339e2b`, native browser identity/session/tenant evidence |
| W00 | [#908](https://github.com/magnus919/SlopSearX/issues/908) | Accepted | [PR #979](https://github.com/magnus919/SlopSearX/pull/979), merged `950071404c61eabc1201730362baccd01b3f2295`; [accepted handoff](https://github.com/magnus919/SlopSearX/issues/908#issuecomment-6218434630), saved definitions and bounded Valkey schedule ownership |
| W01 | [#905](https://github.com/magnus919/SlopSearX/issues/905) | Accepted | [PR #964](https://github.com/magnus919/SlopSearX/pull/964), merged `5a062c884af1d045baeadcf8c7ae1d84ad9deeb3`; [accepted handoff](https://github.com/magnus919/SlopSearX/issues/905#issuecomment-6207591459), conserved research state and accepted F11 identity |
| O03 | [#920](https://github.com/magnus919/SlopSearX/issues/920) | Accepted | [PR #965](https://github.com/magnus919/SlopSearX/pull/965), merged `5395518939cf4f48ee9063e950b8ad4363503143`; active experiment tooling inventory and prospective native evidence contracts; [accepted handoff](https://github.com/magnus919/SlopSearX/issues/920#issuecomment-6206715399); no experiment or provider execution |
| O06 | [#935](https://github.com/magnus919/SlopSearX/issues/935) | Accepted | [PR #958](https://github.com/magnus919/SlopSearX/pull/958), merged `f539c0c2f597cd2fcfc382d343d08465baf11283`; exact-head CI and independent/Droid reviews passed; O09 owns gate activation |

All other packages remain pending. [The dependency index](ISSUES.md) and native
GitHub dependencies determine readiness. Update this ledger with merged PR and
handoff evidence as packages are accepted; retain failed checks and corrections.

## Completion audit

At G09, rederive requirements from current issues, accepted ADRs, capability and
Python-disposition manifests, the case study and maintainer instructions. For
each requirement, record direct evidence or mark it incomplete. Verify all
public surfaces, adapters, state/recovery behavior, browser journeys, native
clients/tooling, exact release artifacts and deployed processes/schedules.
Tests and static manifests only prove the cases they actually cover. Close the
epic and milestone only when every requirement has sufficient current evidence.

A04 broad API adapters: independent source review approved; accepted-main integration qualification and parent acceptance remain pending. See [A04 scope and successor handoff](a04/README.md).
