from __future__ import annotations

import hashlib
import importlib.util
import os
import stat
import tempfile
import unittest
import uuid
from pathlib import Path

MODULE_PATH = Path(__file__).resolve().parents[1] / "scripts" / "intent_ranking_receipts.py"
SPEC = importlib.util.spec_from_file_location("intent_ranking_receipts", MODULE_PATH)
receipts = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(receipts)


def sha(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


class IntentRankingReceiptTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="intent-ranking-receipts-")
        self.root = Path(self.temp.name)
        self.bindings = {
            "stage_uuid": str(uuid.uuid4()),
            "operation_id": "d1-base",
            "request_body_sha256": sha(b"request bytes"),
            "source_revision": "a" * 40,
        }
        self.body = b"\xff{not-json}\x00"

    def tearDown(self):
        self.temp.cleanup()

    def write(self, *, body=None, complete=True, status="complete", http_status=200):
        return receipts.archive_response(
            self.root,
            bindings=self.bindings,
            complete=complete,
            status=status,
            http_status=http_status,
            response_body=self.body if body is None else body,
        )

    def test_empty_interrupted_slot_cannot_be_repaired(self):
        stage = self.root / self.bindings["stage_uuid"]
        stage.mkdir(mode=0o700)
        slot = stage / self.bindings["operation_id"]
        slot.mkdir(mode=0o700)
        with self.assertRaisesRegex(receipts.ReceiptError, "archive-slot-already-exists"):
            self.write()
        self.assertEqual(list(slot.iterdir()), [])

    def test_missing_http_status_is_preserved_but_never_replayed(self):
        written = self.write(complete=False, status="transport_error", http_status=None)
        slot = self.root / self.bindings["stage_uuid"] / self.bindings["operation_id"]
        self.assertIn(b'"http_status":null', (slot / "receipt.json").read_bytes())
        with self.assertRaisesRegex(receipts.ReceiptError, "response-incomplete"):
            receipts.replay_response(
                self.root, expected_bindings=self.bindings, expected_receipt_sha256=written["receipt_sha256"]
            )

    def test_exact_bytes_round_trip_without_decoding(self):
        written = self.write()
        receipt, body = receipts.replay_response(
            self.root, expected_bindings=self.bindings, expected_receipt_sha256=written["receipt_sha256"]
        )
        self.assertEqual(body, self.body)
        self.assertEqual(receipt["response_body_sha256"], sha(self.body))
        with self.assertRaises(UnicodeDecodeError):
            body.decode("utf-8")

    def test_binding_and_receipt_pins_are_exact(self):
        written = self.write()
        for field, replacement in (
            ("request_body_sha256", "b" * 64),
            ("stage_uuid", str(uuid.uuid4())),
            ("source_revision", "c" * 40),
        ):
            wrong = dict(self.bindings, **{field: replacement})
            with self.subTest(field=field), self.assertRaises(receipts.ReceiptError):
                receipts.replay_response(
                    self.root, expected_bindings=wrong, expected_receipt_sha256=written["receipt_sha256"]
                )
        with self.assertRaisesRegex(receipts.ReceiptError, "receipt-digest-mismatch"):
            receipts.replay_response(self.root, expected_bindings=self.bindings, expected_receipt_sha256="0" * 64)

    def test_body_tamper_and_malformed_receipt_are_rejected(self):
        written = self.write()
        slot = self.root / self.bindings["stage_uuid"] / self.bindings["operation_id"]
        (slot / "response.bin").write_bytes(b"changed")
        with self.assertRaisesRegex(receipts.ReceiptError, "response-body-digest-mismatch"):
            receipts.replay_response(
                self.root, expected_bindings=self.bindings, expected_receipt_sha256=written["receipt_sha256"]
            )

        # Recreate in a separate root to test receipt parser after a pinned digest.
        other = self.root / "other"
        other.mkdir(mode=0o700)
        self.write_at(other)
        slot = other / self.bindings["stage_uuid"] / self.bindings["operation_id"]
        receipt_path = slot / "receipt.json"
        receipt_path.write_bytes(b"not-json")
        os.chmod(receipt_path, 0o600)
        with self.assertRaisesRegex(receipts.ReceiptError, "invalid-receipt-json"):
            receipts.replay_response(other, expected_bindings=self.bindings, expected_receipt_sha256=sha(b"not-json"))

    def test_symlinked_body_and_archive_root_are_rejected(self):
        written = self.write()
        slot = self.root / self.bindings["stage_uuid"] / self.bindings["operation_id"]
        body_path = slot / "response.bin"
        body_path.unlink()
        outside = self.root / "outside.bin"
        outside.write_bytes(self.body)
        body_path.symlink_to(outside)
        with self.assertRaisesRegex(receipts.ReceiptError, "archive-file-unsafe"):
            receipts.replay_response(
                self.root, expected_bindings=self.bindings, expected_receipt_sha256=written["receipt_sha256"]
            )
        link = self.root.parent / (self.root.name + "-link")
        link.symlink_to(self.root, target_is_directory=True)
        try:
            with self.assertRaisesRegex(receipts.ReceiptError, "archive-root-not-directory"):
                receipts.archive_response(
                    link,
                    bindings=dict(self.bindings, stage_uuid=str(uuid.uuid4())),
                    complete=True,
                    status="complete",
                    http_status=200,
                    response_body=self.body,
                )
        finally:
            link.unlink()

    def write_at(self, root):
        return receipts.archive_response(
            root, bindings=self.bindings, complete=True, status="complete", http_status=200, response_body=self.body
        )

    def test_duplicate_write_never_overwrites_existing_slot(self):
        first = self.write()
        with self.assertRaisesRegex(receipts.ReceiptError, "archive-slot-already-exists"):
            self.write(body=b"replacement")
        _, replayed = receipts.replay_response(
            self.root, expected_bindings=self.bindings, expected_receipt_sha256=first["receipt_sha256"]
        )
        self.assertEqual(replayed, self.body)

    def test_incomplete_failure_and_partial_slots_are_unusable(self):
        written = self.write(complete=False, status="failed", http_status=503)
        with self.assertRaisesRegex(receipts.ReceiptError, "response-incomplete"):
            receipts.replay_response(
                self.root, expected_bindings=self.bindings, expected_receipt_sha256=written["receipt_sha256"]
            )
        self.assertTrue(
            (self.root / self.bindings["stage_uuid"] / self.bindings["operation_id"] / "receipt.json").exists()
        )

        failed_root = self.root / "failed"
        failed_root.mkdir(mode=0o700)
        failed = receipts.archive_response(
            failed_root,
            bindings=self.bindings,
            complete=True,
            status="failed",
            http_status=503,
            response_body=self.body,
        )
        with self.assertRaisesRegex(receipts.ReceiptError, "response-status-incomplete"):
            receipts.replay_response(
                failed_root, expected_bindings=self.bindings, expected_receipt_sha256=failed["receipt_sha256"]
            )

        partial_root = self.root / "partial"
        partial_root.mkdir(mode=0o700)
        stage = partial_root / self.bindings["stage_uuid"]
        stage.mkdir(mode=0o700)
        slot = stage / self.bindings["operation_id"]
        slot.mkdir(mode=0o700)
        body_file = slot / "response.bin"
        body_file.write_bytes(self.body[:3])
        os.chmod(body_file, 0o600)
        with self.assertRaisesRegex(receipts.ReceiptError, "archive-slot-inventory"):
            receipts.replay_response(partial_root, expected_bindings=self.bindings, expected_receipt_sha256="0" * 64)

    def test_overbound_body_fails_before_creating_slot(self):
        with self.assertRaisesRegex(receipts.ReceiptError, "response-body-too-large"):
            self.write(body=b"x" * (receipts.MAX_BODY_BYTES + 1))
        self.assertEqual(list(self.root.iterdir()), [])

    def test_unsafe_ids_and_symlinks_reject(self):
        for operation_id in ("../escape", ".", "bad/name"):
            bindings = dict(self.bindings, operation_id=operation_id)
            with self.subTest(operation_id=operation_id), self.assertRaises(receipts.ReceiptError):
                receipts.archive_response(
                    self.root,
                    bindings=bindings,
                    complete=True,
                    status="complete",
                    http_status=200,
                    response_body=self.body,
                )

        stage = self.root / self.bindings["stage_uuid"]
        stage.symlink_to(self.root, target_is_directory=True)
        with self.assertRaisesRegex(receipts.ReceiptError, "archive-directory-unsafe"):
            self.write()

    def test_private_permissions_and_exact_inventory(self):
        written = self.write()
        stage = self.root / self.bindings["stage_uuid"]
        slot = stage / self.bindings["operation_id"]
        self.assertEqual(stat.S_IMODE(stage.stat().st_mode), 0o700)
        self.assertEqual(stat.S_IMODE(slot.stat().st_mode), 0o700)
        for filename in ("response.bin", "receipt.json"):
            self.assertEqual(stat.S_IMODE((slot / filename).stat().st_mode), 0o600)
        verified = receipts.verify_inventory(
            self.root, [{"bindings": self.bindings, "receipt_sha256": written["receipt_sha256"]}]
        )
        self.assertEqual(verified[0][1], self.body)
        with self.assertRaisesRegex(receipts.ReceiptError, "archive-inventory-mismatch"):
            receipts.verify_inventory(self.root, [])


if __name__ == "__main__":
    unittest.main()
