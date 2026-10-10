from __future__ import annotations

import hashlib
import json
import stat
import tempfile
import threading
import time
import unittest
import uuid
from pathlib import Path

from scripts import coverage_operator_handoff as handoff


def canonical(value: object) -> bytes:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), allow_nan=False).encode()


def digest(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


class OperatorReceiptHandoffTests(unittest.TestCase):
    def test_waits_for_separate_operator_receipt_and_pin(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "handoff"
            exchange = handoff.OperatorReceiptHandoff(root, poll_interval=0.02)
            stage = str(uuid.UUID(int=8001))
            bindings = {"source_revision": "a" * 40, "protocol_sha256": "b" * 64, "input_sha256": "c" * 64}
            result: list[object] = []

            def run_request():
                try:
                    result.append(
                        exchange.request(
                            stage_uuid=stage,
                            scope="selector-admission",
                            request_id="late-map-1",
                            bindings=bindings,
                            deadline_monotonic=time.monotonic() + 2,
                        )
                    )
                except Exception as exc:  # surfaced in assertion below
                    result.append(exc)

            worker = threading.Thread(target=run_request)
            worker.start()
            request_path = root / stage / "selector-admission" / "requests" / "late-map-1.json"
            for _ in range(100):
                if request_path.exists():
                    break
                time.sleep(0.005)
            envelope = json.loads(request_path.read_bytes())
            self.assertEqual(envelope["bindings"], bindings)
            self.assertEqual(envelope["request_sha256"], digest(canonical(bindings)))
            self.assertEqual(stat.S_IMODE(request_path.stat().st_mode), 0o600)
            self.assertEqual(stat.S_IMODE(root.stat().st_mode), 0o700)
            receipt = canonical({"externally_issued": True, "opaque": "receipt-bytes"})
            response = root / stage / "selector-admission" / "responses" / "late-map-1.receipt"
            pin = root / stage / "selector-admission" / "pins" / "late-map-1.sha256"
            response.write_bytes(receipt)
            response.chmod(0o600)
            pin.write_text(digest(receipt) + "\n", encoding="ascii")
            pin.chmod(0o600)
            worker.join(timeout=2)
            self.assertFalse(worker.is_alive())
            self.assertEqual(result, [(receipt, digest(receipt))])

    def test_bad_pin_is_terminal_and_request_cannot_be_reissued(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "handoff"
            exchange = handoff.OperatorReceiptHandoff(root, poll_interval=0.02)
            stage = str(uuid.UUID(int=8002))
            response = root / stage / "capture" / "responses" / "receipt-1.receipt"
            pin = root / stage / "capture" / "pins" / "receipt-1.sha256"
            (root / stage / "capture" / "responses").mkdir(parents=True, mode=0o700)
            (root / stage / "capture").chmod(0o700)
            (root / stage).chmod(0o700)
            (root / stage / "capture" / "pins").mkdir(mode=0o700)
            response.write_bytes(b"receipt")
            response.chmod(0o600)
            pin.write_text("0" * 64, encoding="ascii")
            pin.chmod(0o600)
            kwargs = {
                "stage_uuid": stage,
                "scope": "capture",
                "request_id": "receipt-1",
                "bindings": {"manifest_sha256": "a" * 64},
                "deadline_monotonic": time.monotonic() + 1,
            }
            with self.assertRaisesRegex(handoff.OperatorHandoffError, "handoff-pin-mismatch"):
                exchange.request(**kwargs)
            with self.assertRaisesRegex(handoff.OperatorHandoffError, "handoff-request-already-created-no-resume"):
                exchange.request(**kwargs)

    def test_timeout_leaves_request_and_never_claims_receipt(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "handoff"
            exchange = handoff.OperatorReceiptHandoff(root, poll_interval=0.02)
            stage = str(uuid.UUID(int=8003))
            with self.assertRaisesRegex(handoff.OperatorHandoffError, "handoff-receipt-timeout-no-resume"):
                exchange.request(
                    stage_uuid=stage,
                    scope="answer",
                    request_id="answer-1",
                    bindings={"operation_manifest_sha256": "d" * 64},
                    deadline_monotonic=time.monotonic() + 0.07,
                )
            self.assertTrue((root / stage / "answer" / "requests" / "answer-1.json").is_file())

    def test_scope_request_and_deadline_are_strict(self):
        with tempfile.TemporaryDirectory() as temporary:
            exchange = handoff.OperatorReceiptHandoff(Path(temporary) / "handoff")
            base = {
                "stage_uuid": str(uuid.UUID(int=8004)),
                "scope": "answer",
                "request_id": "safe-id",
                "bindings": {"x": 1},
                "deadline_monotonic": time.monotonic() + 1,
            }
            for change in (
                {"scope": "../escape"},
                {"request_id": "../escape"},
                {"deadline_monotonic": time.monotonic() + handoff.MAX_WAIT_SECONDS + 1},
                {"bindings": {"bad": float("nan")}},
                {"bindings": {"api_key": "never-persist"}},
            ):
                with self.subTest(change=change), self.assertRaises(handoff.OperatorHandoffError):
                    exchange.request(**(base | change))


if __name__ == "__main__":
    unittest.main()
