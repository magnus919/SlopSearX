# Native-host grader dispatch

The registered stage's native grader handoff writes immutable request packets
under `<grader_handoff>/<packet_stage_uuid>/<phase>/`. For a fresh operator-runner
stage, `NativeGraderHandoff` requires a host transcript for every result before
it returns any `GradeSubmission`. The exact `coverage-native-host-transcript/1`
records are checked against the request packet bytes, model, stage, phase,
thread, completed turn, and exact final response bytes. The closed handoff
receipt records each transcript digest. Missing, extra, malformed, or mismatched
records stop the phase without partial submissions or retry.

After the handoff manifest and requests exist, run the host dispatcher once for
that phase, using the same private phase directory and the pinned Codex CLI:

```sh
python -m scripts.coverage_native_host_dispatch \
  --scope /absolute/private/grader-handoff/PACKET_STAGE_UUID/references \
  --stage-uuid PACKET_STAGE_UUID \
  --phase references \
  --codex-executable /absolute/pinned/path/to/codex \
  --codex-sha256 OUT_OF_BAND_CLI_SHA256 \
  --deadline-seconds REMAINING_REGISTERED_STAGE_SECONDS
```

Use `--phase answers` for the answer-assessment packet handoff. The dispatcher
reserves each packet before starting its app-server process, permits at most two
concurrent turns, disables provider-model fallback, and requires the returned
model to match `gpt-6.1-sol`. An absent exact-model row in `model/list` is retained
as catalog metadata; it does not substitute a different model. If explicit
thread setup or a turn fails, the packet remains terminal and cannot be retried.

The Codex CLI must use the operator's existing authorized host configuration.
The dispatcher pins the executable bytes but does not inspect or export
credentials. Tests use only a synthetic app-server executable and make no model
calls. This dispatcher and its offline tests are not source qualification or
provider-availability evidence.

Transcript files are local host records, not provider-signed attestations. The
runner requires these records in addition to the result envelope's
operator-attested fields; the result-envelope hash alone does not prove a call.
The model-list catalog is informational and cannot prove a model route. A real
fresh stage still requires the separate registered source, capture, protocol,
and execution permits, plus the applicable stage gates.
