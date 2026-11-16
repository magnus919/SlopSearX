"""Synthetic-only durability checks for exact assessment checkpoint records."""

from __future__ import annotations

import hashlib
import json
import os
from dataclasses import replace
from pathlib import Path

import pytest

from scripts import coverage_assessment_checkpoint as checkpoint
from scripts import coverage_grade_closure as grades
from tests.test_coverage_assessment_packets import _fixture
from tests.test_coverage_grade_closure import _pre_pins, _pre_submissions


def _sha(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


def _private_root(path: Path) -> Path:
    path.mkdir(mode=0o700)
    os.chmod(path, 0o700)
    return path


def test_prepared_and_validated_closure_are_persisted_exactly(tmp_path: Path) -> None:
    prepared, _captures = _fixture()
    root = _private_root(tmp_path / "checkpoint")

    saved = checkpoint.persist_prepared_assessment(root, prepared)
    prepared_dir = saved.directory / "prepared"
    index_raw = (prepared_dir / "prepared-index.json").read_bytes()
    assert _sha(index_raw) == saved.index_sha256
    assert (prepared_dir / "private-binding.json").read_bytes() == prepared.private_binding_bytes
    assert (prepared_dir / "preassessment-manifest.json").read_bytes() == prepared.preassessment_manifest_bytes
    index = json.loads(index_raw)
    assert index["private_binding_sha256"] == prepared.private_binding_sha256
    assert index["manifest_sha256"] == _sha(prepared.preassessment_manifest_bytes)
    assert index["packet_count"] == len(prepared.preassessment_packets)
    for packet in prepared.preassessment_packets:
        saved_packet = prepared_dir / "packets" / f"{packet['packet_id']}.json"
        assert saved_packet.read_bytes() == packet["bytes"]
        assert _sha(saved_packet.read_bytes()) == packet["sha256"]

    submissions = _pre_submissions(prepared)
    closure = grades.close_preassessment(prepared, submissions, **_pre_pins(prepared))
    closure_sha = checkpoint.persist_validated_preassessment_closure(saved, prepared, submissions, closure)
    closed_dir = saved.directory / "closed"
    closure_index_raw = (closed_dir / "closure-index.json").read_bytes()
    closure_index = json.loads(closure_index_raw)
    assert _sha(closure_index_raw) == closure_sha
    assert closure_index["prepared_index_sha256"] == saved.index_sha256
    assert (closed_dir / "grade-closure-receipt.json").read_bytes() == closure.receipt_bytes
    assert (closed_dir / "closed-outputs.json").read_bytes() == closure._outputs_bytes
    assert (closed_dir / "closed-source-outputs.json").read_bytes() == closure._source_outputs_bytes
    assert len(list((closed_dir / "submissions").glob("*.response.json"))) == len(submissions)


def test_partial_prepared_record_never_returns_checkpoint_capability(tmp_path: Path, monkeypatch) -> None:
    prepared, _captures = _fixture()
    root = _private_root(tmp_path / "checkpoint")
    write_new = checkpoint._write_new

    def fail_before_index(path: Path, raw: bytes, *, maximum_bytes: int) -> str:
        if path.name == "preassessment-manifest.json":
            raise OSError("synthetic disk failure")
        return write_new(path, raw, maximum_bytes=maximum_bytes)

    monkeypatch.setattr(checkpoint, "_write_new", fail_before_index)
    with pytest.raises(OSError, match="synthetic disk failure"):
        checkpoint.persist_prepared_assessment(root, prepared)
    prepared_dir = root / prepared.packet_stage_uuid / "prepared"
    assert not (prepared_dir / "prepared-index.json").exists()
    assert (prepared_dir / "private-binding.json").exists()


def test_checkpoint_refuses_reuse_tampering_and_symlink_roots(tmp_path: Path) -> None:
    prepared, _captures = _fixture()
    root = _private_root(tmp_path / "checkpoint")
    saved = checkpoint.persist_prepared_assessment(root, prepared)
    with pytest.raises(checkpoint.AssessmentCheckpointError, match="already-exists-no-resume"):
        checkpoint.persist_prepared_assessment(root, prepared)

    private_binding = saved.directory / "prepared" / "private-binding.json"
    original = private_binding.read_bytes()
    private_binding.write_bytes(original + b" ")
    os.chmod(private_binding, 0o600)
    submissions = _pre_submissions(prepared)
    closure = grades.close_preassessment(prepared, submissions, **_pre_pins(prepared))
    with pytest.raises(checkpoint.AssessmentCheckpointError, match="artifact-mismatch"):
        checkpoint.persist_validated_preassessment_closure(saved, prepared, submissions, closure)
    private_binding.write_bytes(original)
    os.chmod(private_binding, 0o600)

    target = tmp_path / "real-root"
    _private_root(target)
    alias = tmp_path / "root-alias"
    alias.symlink_to(target, target_is_directory=True)
    with pytest.raises(checkpoint.AssessmentCheckpointError, match="directory-unsafe"):
        checkpoint.persist_prepared_assessment(alias, prepared)


def test_packet_symlink_and_oversize_artifact_are_rejected(tmp_path: Path) -> None:
    prepared, _captures = _fixture()
    root = _private_root(tmp_path / "checkpoint")
    saved = checkpoint.persist_prepared_assessment(root, prepared)
    packet_path = saved.directory / "prepared" / "packets" / f"{prepared.preassessment_packets[0]['packet_id']}.json"
    backup_path = packet_path.with_suffix(".saved")
    packet_path.rename(backup_path)
    target = tmp_path / "symlink-target.json"
    target.write_bytes(backup_path.read_bytes())
    packet_path.symlink_to(target)
    submissions = _pre_submissions(prepared)
    closure = grades.close_preassessment(prepared, submissions, **_pre_pins(prepared))
    with pytest.raises(checkpoint.AssessmentCheckpointError, match="artifact-unavailable"):
        checkpoint.persist_validated_preassessment_closure(saved, prepared, submissions, closure)
    packet_path.unlink()
    backup_path.rename(packet_path)

    bounded = root / "bounded.json"
    with pytest.raises(checkpoint.AssessmentCheckpointError, match="size-invalid"):
        checkpoint._write_new(bounded, b"too large", maximum_bytes=2)
    assert not bounded.exists()


def test_closure_must_be_revalidated_against_exact_prepared_bytes(tmp_path: Path) -> None:
    prepared, _captures = _fixture()
    root = _private_root(tmp_path / "checkpoint")
    saved = checkpoint.persist_prepared_assessment(root, prepared)
    submissions = _pre_submissions(prepared)
    closure = grades.close_preassessment(prepared, submissions, **_pre_pins(prepared))
    changed = replace(closure, receipt_bytes=closure.receipt_bytes + b" ")
    with pytest.raises(checkpoint.AssessmentCheckpointError, match="closure-binding-invalid"):
        checkpoint.persist_validated_preassessment_closure(saved, prepared, submissions, changed)
    assert not (saved.directory / "closed").exists()
