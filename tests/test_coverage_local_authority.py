from __future__ import annotations

import hashlib
import json
import stat
import tempfile
import unittest
import uuid
from pathlib import Path

from scripts import coverage_answer_execution as answer
from scripts import coverage_live_acquire as acquisition
from scripts import coverage_local_authority as authority
from scripts import coverage_source_capture as capture


def canonical(value: object) -> bytes:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False).encode()


def digest(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def receipt(scope: str, bindings: dict[str, object], *, evidence=None) -> bytes:
    value = {
        "schema": authority.SCHEMA,
        "decision": "allow",
        "scope": scope,
        "bindings": bindings,
    }
    if evidence is not None:
        value["evidence"] = evidence
    return canonical(value)


class PinnedReceiptAuthorityTests(unittest.TestCase):
    def test_acquisition_verifier_requires_pinned_exact_current_bindings(self):
        manifest = {
            "stage_uuid": str(uuid.UUID(int=1001)),
            "source_revision": "a" * 40,
            "source_closure_sha256": "1" * 64,
            "protocol_sha256": "2" * 64,
            "cohorts_sha256": "3" * 64,
            "input_manifest_sha256": "4" * 64,
            "acquisition_plan_sha256": "5" * 64,
        }
        manifest_sha = "6" * 64
        bindings = {**manifest, "stage_manifest_sha256": manifest_sha}
        raw = receipt("source-acquisition", bindings)
        permit = authority.PinnedReceiptAuthorities.acquisition(manifest, manifest_sha, raw, digest(raw))
        self.assertIsInstance(permit, acquisition.VerifiedAcquisitionPermit)
        self.assertEqual(
            authority.AcquisitionReceiptVerifier().verify(manifest, manifest_sha, raw, digest(raw)), permit
        )
        self.assertEqual(permit.receipt_sha256, digest(raw))
        changed = dict(bindings, protocol_sha256="f" * 64)
        changed_raw = receipt("source-acquisition", changed)
        with self.assertRaisesRegex(authority.LocalAuthorityError, "receipt-claims-mismatch"):
            authority.PinnedReceiptAuthorities.acquisition(manifest, manifest_sha, changed_raw, digest(changed_raw))
        with self.assertRaisesRegex(authority.LocalAuthorityError, "receipt-pin-or-binding-invalid"):
            authority.PinnedReceiptAuthorities.acquisition(manifest, manifest_sha, raw, "0" * 64)

    def test_capture_receipt_matches_manifest_and_frozen_limits(self):
        manifest = {
            "stage_uuid": str(uuid.UUID(int=1002)),
            "source_revision": "b" * 40,
            "protocol_sha256": "1" * 64,
            "cohorts_sha256": "2" * 64,
            "candidate_identity_sha256": "3" * 64,
            "candidate_endpoint_sha256": "4" * 64,
            "sources": [{"source_id": "s1"}],
        }
        manifest_bytes = canonical(manifest)
        bindings = {
            "stage_uuid": manifest["stage_uuid"],
            "manifest_sha256": digest(manifest_bytes),
            "protocol_sha256": manifest["protocol_sha256"],
            "source_revision": manifest["source_revision"],
            "cohorts_sha256": manifest["cohorts_sha256"],
            "candidate_identity_sha256": manifest["candidate_identity_sha256"],
            "candidate_endpoint_sha256": manifest["candidate_endpoint_sha256"],
            "source_count": 1,
            "max_owned_calls": capture.MAX_OWNED_CALLS,
            "max_health_calls": capture.MAX_HEALTH_CALLS,
            "max_scrape_calls": capture.MAX_SCRAPE_CALLS,
            "timeout_seconds": capture.REQUEST_TIMEOUT_SECONDS,
            "response_bytes": capture.MAX_RESPONSE_BYTES,
            "source_context_characters": capture.MAX_SOURCE_CHARS,
            "ca_bundle_sha256": None,
        }
        raw = receipt("source-capture", bindings)
        permit = authority.PinnedReceiptAuthorities.source_capture(manifest_bytes, raw, digest(raw))
        self.assertEqual(permit.source_count, 1)
        self.assertEqual(authority.SourceCaptureReceiptVerifier().verify(manifest_bytes, raw, digest(raw)), permit)
        custom_ca = "a" * 64
        custom_bindings = dict(bindings, ca_bundle_sha256=custom_ca)
        custom_raw = receipt("source-capture", custom_bindings)
        custom_permit = authority.PinnedReceiptAuthorities.source_capture(
            manifest_bytes, custom_raw, digest(custom_raw), custom_ca
        )
        self.assertEqual(custom_permit.ca_bundle_sha256, custom_ca)
        with self.assertRaisesRegex(authority.LocalAuthorityError, "receipt-claims-mismatch"):
            authority.PinnedReceiptAuthorities.source_capture(manifest_bytes, custom_raw, digest(custom_raw), None)
        changed = dict(bindings, response_bytes=capture.MAX_RESPONSE_BYTES - 1)
        with self.assertRaisesRegex(authority.LocalAuthorityError, "receipt-claims-mismatch"):
            authority.PinnedReceiptAuthorities.source_capture(
                manifest_bytes,
                receipt("source-capture", changed),
                digest(receipt("source-capture", changed)),
            )

    def test_capture_qualification_requires_exact_source_binding_and_evidence_shape(self):
        bindings = capture.ProtectedCaptureQualificationBindings(
            stage_uuid=str(uuid.UUID(int=1003)),
            source_revision="c" * 40,
            protocol_sha256="1" * 64,
            candidate_identity_sha256="2" * 64,
            candidate_endpoint_sha256="3" * 64,
            candidate_endpoint_scheme="https",
            candidate_runtime_revision="d" * 40,
            capture_module_sha256="4" * 64,
        )
        evidence = {
            "grok_image_digest": "5" * 64,
            "grok_config_sha256": "6" * 64,
            "protected_profile_sha256": "7" * 64,
            "full_boundary_evidence_sha256": "8" * 64,
        }
        raw = receipt("protected-source-capture", bindings.__dict__, evidence=evidence)
        verified = authority.PinnedReceiptAuthorities.protected_capture_qualification(raw, digest(raw), bindings)
        self.assertEqual(verified.status, "verified-qualified")
        self.assertEqual(verified.bindings, bindings)
        self.assertEqual(authority.ProtectedCaptureReceiptVerifier().verify(raw, digest(raw), bindings), verified)
        modified = dict(bindings.__dict__, candidate_endpoint_scheme="http")
        changed_raw = receipt("protected-source-capture", modified, evidence=evidence)
        with self.assertRaisesRegex(authority.LocalAuthorityError, "receipt-claims-mismatch"):
            authority.PinnedReceiptAuthorities.protected_capture_qualification(
                changed_raw, digest(changed_raw), bindings
            )
        with self.assertRaisesRegex(authority.LocalAuthorityError, "capture-qualification-evidence-invalid"):
            authority.PinnedReceiptAuthorities.protected_capture_qualification(
                receipt("protected-source-capture", bindings.__dict__, evidence={"grok_image_digest": "x"}),
                digest(receipt("protected-source-capture", bindings.__dict__, evidence={"grok_image_digest": "x"})),
                bindings,
            )

    def test_answer_receipt_binds_exact_endpoint_operations_and_caps(self):
        bindings = {
            "stage_uuid": str(uuid.UUID(int=1004)),
            "source_revision": "e" * 40,
            "protocol_sha256": "1" * 64,
            "cohorts_sha256": "2" * 64,
            "operation_manifest_sha256": "3" * 64,
            "endpoint_sha256": "4" * 64,
            "resolved_destination_sha256": "5" * 64,
            "endpoint_security_mode": "https-required",
            "operation_ids": ["answer-D-R01-w0", "answer-D-R01-candidate"],
        }
        evidence = {
            "max_calls": answer.MAX_CALLS,
            "request_bytes_maximum": answer.MAX_REQUEST_BYTES,
            "response_bytes_maximum": answer.MAX_RESPONSE_BYTES,
        }
        raw = receipt("answer-execution", bindings, evidence=evidence)
        permit = authority.PinnedReceiptAuthorities.answer(raw, digest(raw), bindings)
        self.assertIsInstance(permit, answer.VerifiedAnswerPermit)
        self.assertEqual(authority.AnswerReceiptVerifier().verify(raw, digest(raw), bindings), permit)
        self.assertEqual(permit.operation_ids, tuple(bindings["operation_ids"]))
        changed = dict(bindings, resolved_destination_sha256="f" * 64)
        with self.assertRaisesRegex(authority.LocalAuthorityError, "receipt-claims-mismatch"):
            authority.PinnedReceiptAuthorities.answer(
                receipt("answer-execution", changed, evidence=evidence),
                digest(receipt("answer-execution", changed, evidence=evidence)),
                bindings,
            )


class FileOneShotLeaseTests(unittest.TestCase):
    def test_claim_is_private_durable_and_cannot_be_reused(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "leases"
            lease = authority.FileOneShotLease(root, scope="answer")
            permit = type("Permit", (), {"stage_uuid": str(uuid.UUID(int=1005)), "receipt_sha256": "a" * 64})()
            first = lease.consume_once(permit, operation_id="answer-D-R01-w0", request_sha256="b" * 64)
            self.assertEqual(first["status"], "consumed")
            rows = list(root.glob("*.lease.json"))
            self.assertEqual(len(rows), 1)
            self.assertEqual(stat.S_IMODE(rows[0].stat().st_mode), 0o600)
            self.assertEqual(stat.S_IMODE(root.stat().st_mode), 0o700)
            content = rows[0].read_bytes()
            self.assertNotIn(b"secret", content)
            with self.assertRaisesRegex(authority.LocalAuthorityError, "lease-already-consumed"):
                lease.consume_once(permit, operation_id="answer-D-R01-w0", request_sha256="b" * 64)
            lease.consume_once(permit, operation_id="answer-D-R01-candidate", request_sha256="c" * 64)
            self.assertEqual(len(list(root.glob("*.lease.json"))), 2)

    def test_changed_request_or_permit_cannot_reclaim_logical_operation(self):
        with tempfile.TemporaryDirectory() as temporary:
            lease = authority.FileOneShotLease(Path(temporary) / "leases", scope="selector")
            stage = str(uuid.UUID(int=1007))
            first = type("Permit", (), {"stage_uuid": stage, "receipt_sha256": "e" * 64})()
            changed = type("Permit", (), {"stage_uuid": stage, "receipt_sha256": "f" * 64})()
            lease.consume_once(first, operation_id="research-D1", request_sha256="1" * 64)
            for permit, request_sha in ((first, "2" * 64), (changed, "1" * 64)):
                with self.subTest(permit=permit.receipt_sha256, request=request_sha):
                    with self.assertRaisesRegex(authority.LocalAuthorityError, "lease-already-consumed"):
                        lease.consume_once(permit, operation_id="research-D1", request_sha256=request_sha)

    def test_symlinked_lease_root_is_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary)
            target = base / "target"
            target.mkdir(mode=0o700)
            link = base / "link"
            link.symlink_to(target, target_is_directory=True)
            with self.assertRaisesRegex(authority.LocalAuthorityError, "lease-root-not-private"):
                authority.FileOneShotLease(link, scope="selector")

    def test_replaced_lease_root_is_rejected_before_claim_write(self):
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary)
            root = base / "leases"
            lease = authority.FileOneShotLease(root, scope="answer")
            root.rmdir()
            target = base / "other"
            target.mkdir(mode=0o700)
            root.symlink_to(target, target_is_directory=True)
            permit = type("Permit", (), {"stage_uuid": str(uuid.UUID(int=1006)), "receipt_sha256": "c" * 64})()
            with self.assertRaisesRegex(authority.LocalAuthorityError, "lease-root-changed"):
                lease.consume_once(permit, operation_id="answer-D-R01-w0", request_sha256="d" * 64)
            self.assertEqual(list(target.iterdir()), [])


if __name__ == "__main__":
    unittest.main()
