"""Offline process tests; no study, provider, search or model execution."""

import importlib.util
import json
import pathlib
import sys
import uuid
from unittest import mock

import pytest

spec = importlib.util.spec_from_file_location(
    "external_invocation", str(pathlib.Path(__file__).resolve().parents[1] / "scripts/coverage_external_invocation.py")
)
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


def fixture(tmp_path):
    source = tmp_path / "source"
    source.mkdir()
    (source / "scripts").mkdir()
    (source / ".venv/bin").mkdir(parents=True)
    (source / ".venv/bin/python").symlink_to(sys.executable)
    (source / "scripts/coverage_operator_launcher.py").write_text("import time\ntime.sleep(60)\n")
    root = tmp_path / "output"
    root.mkdir(mode=0o700)
    stage = str(uuid.uuid4())
    config = tmp_path / "config.json"
    data = {
        "stage_uuid": stage,
        "packet_stage_uuid": str(uuid.uuid4()),
        "forbidden_stage_uuids": [],
        "source_revision": "a" * 40,
        "directories": {},
    }
    raw = module.canonical(data)
    config.write_bytes(raw)
    config.chmod(0o600)
    args = [
        "invocation",
        "--source",
        str(source),
        "--source-revision",
        "a" * 40,
        "--config",
        str(config),
        "--config-sha256",
        module.hashlib.sha256(raw).hexdigest(),
        "--output-root",
        str(root),
        "--issuer-script",
        str(tmp_path / "issuer.py"),
    ]
    return source, root, stage, args


def preflight(command, **kwargs):
    assert kwargs["cwd"].name == "source"
    assert "--preflight-only" in command
    status = "issuer-static-preflight-passed" if "-I" in command else "static-preflight-passed"
    return module.subprocess.CompletedProcess(command, 0, stdout=json.dumps({"status": status}).encode(), stderr=b"")


def test_controller_failure_terminates_real_waiting_process_and_retains_terminal(tmp_path):
    source, root, stage, args = fixture(tmp_path)
    (root / (stage + ".controller-failure.json")).write_text('{"exit_code":1}')
    with (
        mock.patch.object(sys, "argv", args),
        mock.patch.object(module.subprocess, "check_output", side_effect=["a" * 40 + "\n", ""]),
        mock.patch.object(module.subprocess, "run", side_effect=preflight),
    ):
        assert module.main() != 0
    terminal = json.loads((root / (stage + ".terminal.json")).read_bytes())
    assert terminal["error_class"] == "ControllerFailure"
    assert terminal["exit_code"] != 0 and terminal["no_retry"] is True
    assert (root / (stage + ".invocation.json")).exists()


def test_preflight_failure_never_takes_invocation_marker_or_starts_child(tmp_path):
    source, root, stage, args = fixture(tmp_path)
    with (
        mock.patch.object(sys, "argv", args),
        mock.patch.object(module.subprocess, "check_output", side_effect=["a" * 40 + "\n", ""]),
        mock.patch.object(
            module.subprocess, "run", return_value=module.subprocess.CompletedProcess([], 1, stdout=b"", stderr=b"")
        ),
        mock.patch.object(module.subprocess, "Popen") as child,
    ):
        with pytest.raises(ValueError, match="registered-static-preflight-refused"):
            module.main()
        child.assert_not_called()
    assert not list(root.iterdir())


def test_existing_invocation_cannot_launch_second_child(tmp_path):
    source, root, stage, args = fixture(tmp_path)
    (root / (stage + ".invocation.json")).write_text("{}")
    with (
        mock.patch.object(sys, "argv", args),
        mock.patch.object(module.subprocess, "check_output", side_effect=["a" * 40 + "\n", ""]),
        mock.patch.object(module.subprocess, "run", side_effect=preflight),
        mock.patch.object(module.subprocess, "Popen") as child,
    ):
        with pytest.raises(FileExistsError):
            module.main()
        child.assert_not_called()
