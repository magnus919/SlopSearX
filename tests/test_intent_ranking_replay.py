from __future__ import annotations

import hashlib
import json
import shutil
import tempfile
import uuid
from pathlib import Path
from unittest.mock import patch

import pytest

from scripts import intent_ranking_coverage as coverage
from scripts import intent_ranking_receipts as receipts
from scripts import intent_ranking_replay as replay
from slopsearx.rerank import LEVELS, MODEL

FACETS = [
    {"id": "scope", "description": "Evidence about requested scope."},
    {"id": "risk", "description": "Evidence about risk."},
]
SOURCE_REVISION = "a" * 40


def sha(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


def canonical(value) -> bytes:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False).encode()


def make_inputs(operation_id="d1-base"):
    candidates = [
        {"id": "a", "title": "Card A", "url": "https://example.test/a", "snippet": "A"},
        {"id": "b", "title": "Card B", "url": "https://example.test/b", "snippet": "B"},
    ]
    incumbent = ["b", "a"]
    return {
        "operation_id": operation_id,
        "query": "public query",
        "purpose": "compare evidence",
        "facets": FACETS,
        "candidates": candidates,
        "incumbent_order": incumbent,
    }


def good_response(compiled):
    answers = {}
    for card in compiled.candidates:
        cid = card["id"]
        probs = {str(i): float(i == 7) for i in range(10)}
        answers[f"score:{cid}"] = {
            "type": "score",
            "score": 7,
            "legend": {str(i): level for i, level in enumerate(LEVELS)},
            "probabilities": probs,
            "confidence": 0.7,
        }
        qid = f"profile:{cid}"
        option = "facets:risk,scope"
        cprobs = {choice: float(choice == option) for choice in compiled.profile_options[qid]}
        answers[qid] = {"type": "choice", "choice": option, "probabilities": cprobs, "confidence": 0.8}
    return canonical({"model": MODEL, "answers": answers, "usage": {"input_tokens": 20, "output_tokens": 8}})


class Fixture:
    def __init__(self, bodies=None):
        self.temp = tempfile.TemporaryDirectory(prefix="intent-ranking-replay-")
        self.root = Path(self.temp.name)
        self.archive = self.root / "archive"
        self.archive.mkdir(mode=0o700)
        self.stage_uuid = str(uuid.uuid4())
        self.rows = []
        self.raw_bodies = {}
        for operation_id in ("d1-base", "d2-base"):
            args = make_inputs(operation_id)
            compiled = coverage.compile_request(
                query=args["query"],
                purpose=args["purpose"],
                facets=args["facets"],
                candidates=args["candidates"],
                incumbent_order=args["incumbent_order"],
            )
            body = (bodies or {}).get(operation_id, good_response(compiled))
            binding = {
                "stage_uuid": self.stage_uuid,
                "operation_id": operation_id,
                "request_body_sha256": sha(compiled.body),
                "source_revision": SOURCE_REVISION,
            }
            receipt = receipts.archive_response(
                self.archive, bindings=binding, complete=True, status="complete", http_status=200, response_body=body
            )
            self.rows.append(
                {**args, "request_body_sha256": sha(compiled.body), "receipt_sha256": receipt["receipt_sha256"]}
            )
            self.raw_bodies[operation_id] = body
        self.manifest = {
            "schema": replay.MANIFEST_SCHEMA,
            "stage_uuid": self.stage_uuid,
            "source_revision": SOURCE_REVISION,
            "operations": self.rows,
        }
        self.manifest_bytes = canonical(self.manifest)
        self.manifest_sha256 = sha(self.manifest_bytes)

    def run(self, **overrides):
        kwargs = {
            "archive_root": self.archive,
            "manifest_bytes": self.manifest_bytes,
            "expected_manifest_sha256": self.manifest_sha256,
            "expected_stage_uuid": self.stage_uuid,
            "expected_source_revision": SOURCE_REVISION,
            "expected_operation_ids": ["d1-base", "d2-base"],
        }
        kwargs.update(overrides)
        return replay.replay_manifest(**kwargs)

    def close(self):
        self.temp.cleanup()


@pytest.fixture
def fx():
    fixture = Fixture()
    yield fixture
    fixture.close()


def test_pinned_complete_inventory_replays_exact_bytes_through_original_parser(fx):
    seen = []
    original = coverage.parse_response

    def capture(raw, compiled):
        seen.append(raw)
        return original(raw, compiled)

    with patch.object(coverage, "parse_response", side_effect=capture):
        result = fx.run()
    assert result["schema"] == "intent-ranking-offline-replay-result/1"
    assert result["status"] == "diagnostic_only"
    assert len(result["results"]) == 2
    assert [row["operation_id"] for row in result["results"]] == ["d1-base", "d2-base"]
    assert all(row["status"] == "selected" for row in result["results"])
    assert seen == [fx.raw_bodies["d1-base"], fx.raw_bodies["d2-base"]]
    assert len(result["result_sha256"]) == 64


def test_malformed_archived_response_is_whole_incumbent_diagnostic(fx):
    malformed = Fixture(bodies={"d1-base": b"\xffnot-json"})
    try:
        result = malformed.run()
        row = result["results"][0]
        assert result["status"] == "diagnostic_only"
        assert row["status"] == "fallback"
        assert row["reason"] == "invalid_response"
        assert row["ordered_ids"] == ["b", "a"]
    finally:
        malformed.close()


def test_manifest_and_request_pins_fail_before_archive_read(fx):
    with patch.object(replay.receipts, "verify_inventory", side_effect=AssertionError("read archive")):
        with pytest.raises(replay.ReplayError, match="manifest-digest-mismatch"):
            fx.run(expected_manifest_sha256="0" * 64)

        bad_rows = [dict(row) for row in fx.rows]
        bad_rows[1]["request_body_sha256"] = "0" * 64
        bad = canonical({**fx.manifest, "operations": bad_rows})
        with pytest.raises(replay.ReplayError, match="manifest-digest-mismatch"):
            fx.run(manifest_bytes=bad)

    bad_rows = [dict(row) for row in fx.rows]
    bad_rows[1]["request_body_sha256"] = "0" * 64
    bad = canonical({**fx.manifest, "operations": bad_rows})
    with patch.object(replay.receipts, "verify_inventory", side_effect=AssertionError("read archive")):
        with pytest.raises(replay.ReplayError, match="request-body-binding-mismatch"):
            fx.run(manifest_bytes=bad, expected_manifest_sha256=sha(bad))


@pytest.mark.parametrize(
    "field,value,error",
    [
        ("expected_stage_uuid", str(uuid.uuid4()), "stage-binding-mismatch"),
        ("expected_source_revision", "b" * 40, "source-revision-binding-mismatch"),
        ("expected_operation_ids", ["d1-base"], "operation-inventory-count"),
        ("expected_operation_ids", ["d1-base", "other"], "operation-inventory-mismatch"),
        ("expected_operation_ids", ["d1-base", "d1-base"], "expected-operation-duplicate"),
        ("expected_operation_ids", ["d2-base", "d1-base"], "operation-inventory-order"),
    ],
)
def test_external_stage_source_and_inventory_pins_are_enforced(fx, field, value, error):
    with pytest.raises(replay.ReplayError, match=error):
        fx.run(**{field: value})


def test_manifest_rows_reject_duplicates_extra_frozen_fields_and_binding_drift(fx):
    duplicate = canonical({**fx.manifest, "operations": [fx.rows[0], fx.rows[0]]})
    with pytest.raises(replay.ReplayError, match="operation-inventory-duplicate"):
        fx.run(manifest_bytes=duplicate, expected_manifest_sha256=sha(duplicate))

    extra_row = dict(fx.rows[0], stage_deadline="not-part-of-manifest-contract")
    extra = canonical({**fx.manifest, "operations": [extra_row, fx.rows[1]]})
    with pytest.raises(replay.ReplayError, match="operation-schema"):
        fx.run(manifest_bytes=extra, expected_manifest_sha256=sha(extra))

    with pytest.raises(replay.ReplayError, match="source-revision-binding-mismatch"):
        fx.run(expected_source_revision="c" * 40)


@pytest.mark.parametrize(
    "raw,error",
    [
        (b'{"schema":"x","schema":"y"}', "manifest-duplicate-key"),
        (b'{"schema":NaN}', "manifest-nonfinite"),
        (b'{"n":' + b"9" * 5000 + b"}", "manifest-json-invalid"),
        (b"{" + b'"x":' * 1200 + b"0" + b"}" * 1200, "manifest-json-invalid"),
    ],
)
def test_strict_manifest_parser_rejects_duplicate_nonfinite_and_deep_json(fx, raw, error):
    with pytest.raises(replay.ReplayError, match=error):
        fx.run(manifest_bytes=raw, expected_manifest_sha256=sha(raw))


def test_receipt_inventory_and_body_tamper_fail_before_any_parser(fx):
    with patch.object(coverage, "parse_response", side_effect=AssertionError("parsed early")):
        shutil.rmtree(fx.archive / fx.stage_uuid / "d2-base")
        with pytest.raises(replay.ReplayError, match="response-archive-invalid"):
            fx.run()

    intact = Fixture()
    slot = intact.archive / intact.stage_uuid / "d1-base" / "response.bin"
    try:
        slot.write_bytes(b"tampered")
        with patch.object(coverage, "parse_response", side_effect=AssertionError("parsed early")):
            with pytest.raises(replay.ReplayError, match="response-archive-invalid"):
                intact.run()
    finally:
        intact.close()


def test_late_operation_input_failure_happens_before_archive_inventory_read(fx):
    bad_rows = [dict(row) for row in fx.rows]
    bad_rows[1]["query"] = ""
    bad = canonical({**fx.manifest, "operations": bad_rows})
    with patch.object(replay.receipts, "verify_inventory", side_effect=AssertionError("read archive")):
        with pytest.raises(replay.ReplayError, match="operation-input-invalid"):
            fx.run(manifest_bytes=bad, expected_manifest_sha256=sha(bad))


def test_result_digest_is_optional_but_checked_when_supplied(fx):
    result = fx.run()
    with pytest.raises(replay.ReplayError, match="result-digest-mismatch"):
        fx.run(expected_result_sha256="0" * 64)
    accepted = fx.run(expected_result_sha256=result["result_sha256"])
    assert accepted["result_sha256"] == result["result_sha256"]
