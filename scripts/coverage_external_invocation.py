"""Start exactly one fresh registered stage; retain its terminal outcome, never retry."""

import argparse
import hashlib
import json
import os
import pathlib
import stat
import subprocess
import time
import uuid


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":")).encode()


def write_new(path, raw):
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
    with os.fdopen(fd, "wb") as f:
        f.write(raw)
        f.flush()
        os.fsync(f.fileno())
    fd = os.open(path.parent, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
    try:
        os.fsync(fd)
    finally:
        os.close(fd)


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--source", required=True, type=pathlib.Path)
    p.add_argument("--source-revision", required=True)
    p.add_argument("--config", required=True, type=pathlib.Path)
    p.add_argument("--config-sha256", required=True)
    p.add_argument("--output-root", required=True, type=pathlib.Path)
    p.add_argument("--issuer-script", required=True, type=pathlib.Path)
    a = p.parse_args()
    source = a.source.resolve(strict=True)
    root = a.output_root
    info = root.lstat()
    if not stat.S_ISDIR(info.st_mode) or info.st_uid != os.geteuid() or stat.S_IMODE(info.st_mode) != 0o700:
        raise ValueError("private-output-root-invalid")
    fd = os.open(a.config, os.O_RDONLY | os.O_NOFOLLOW)
    try:
        info = os.fstat(fd)
        if (
            not stat.S_ISREG(info.st_mode)
            or info.st_uid != os.geteuid()
            or stat.S_IMODE(info.st_mode) != 0o600
            or info.st_size > 2_000_000
        ):
            raise ValueError("private-config-invalid")
        raw = os.read(fd, 2_000_001)
    finally:
        os.close(fd)
    if hashlib.sha256(raw).hexdigest() != a.config_sha256:
        raise ValueError("config-pin-mismatch")
    d = json.loads(raw)
    stage = d["stage_uuid"]
    packet = d["packet_stage_uuid"]
    for identity in (stage, packet):
        parsed = uuid.UUID(identity)
        if str(parsed) != identity or parsed.version != 4 or identity in d["forbidden_stage_uuids"]:
            raise ValueError("fresh-identity-invalid")
    if stage == packet or d["source_revision"] != a.source_revision:
        raise ValueError("stage-bindings-invalid")
    if (
        subprocess.check_output(["git", "-C", str(source), "rev-parse", "HEAD"], text=True).strip() != a.source_revision
        or subprocess.check_output(["git", "-C", str(source), "status", "--porcelain"], text=True).strip()
    ):
        raise ValueError("clean-source-mismatch")
    if any(pathlib.Path(value).exists() for value in d["directories"].values()):
        raise ValueError("fresh-stage-roots-already-exist")
    command = [
        str(source / ".venv/bin/python"),
        str(source / "scripts/coverage_operator_launcher.py"),
        "--config",
        str(a.config),
        "--config-sha256",
        a.config_sha256,
    ]
    checked = subprocess.run(command + ["--preflight-only"], cwd=source, capture_output=True, check=False)
    if checked.returncode or json.loads(checked.stdout).get("status") != "static-preflight-passed":
        raise ValueError("registered-static-preflight-refused")
    issuer_command = [
        str(source / ".venv/bin/python"),
        "-I",
        str(a.issuer_script),
        "--source",
        str(source),
        "--source-revision",
        a.source_revision,
        "--config",
        str(a.config),
        "--config-sha256",
        a.config_sha256,
        "--scope",
        "frozen-pool-collection",
        "--preflight-only",
    ]
    issuer_checked = subprocess.run(issuer_command, cwd=source, capture_output=True, check=False)
    if issuer_checked.returncode or json.loads(issuer_checked.stdout).get("status") != "issuer-static-preflight-passed":
        raise ValueError("actual-issuer-static-preflight-refused")
    write_new(
        root / (stage + ".invocation.json"),
        canonical(
            {
                "schema": "coverage-external-invocation/1",
                "stage_uuid": stage,
                "packet_stage_uuid": packet,
                "source_revision": a.source_revision,
                "config_sha256": a.config_sha256,
                "started_utc_unix": time.time(),
                "invocation": 1,
                "no_retry": True,
            }
        ),
    )
    stdout = root / (stage + ".stdout")
    stderr = root / (stage + ".stderr")
    out_fd = os.open(stdout, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
    err_fd = os.open(stderr, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
    exit_code = None
    error_class = None
    try:
        with os.fdopen(out_fd, "wb") as out, os.fdopen(err_fd, "wb") as err:
            child = subprocess.Popen(command, cwd=source, stdout=out, stderr=err)
            while child.poll() is None:
                if (root / (stage + ".controller-failure.json")).exists():
                    child.terminate()
                    try:
                        child.wait(timeout=10)
                    except subprocess.TimeoutExpired:
                        child.kill()
                        child.wait()
                    error_class = "ControllerFailure"
                    break
                time.sleep(0.1)
            exit_code = child.returncode
            out.flush()
            err.flush()
            os.fsync(out.fileno())
            os.fsync(err.fileno())
    except Exception as exc:
        error_class = type(exc).__name__
    terminal = {
        "stage_uuid": stage,
        "exit_code": exit_code,
        "error_class": error_class,
        "completed_utc_unix": time.time(),
        "no_retry": True,
        "stdout_sha256": hashlib.sha256(stdout.read_bytes()).hexdigest(),
        "stderr_sha256": hashlib.sha256(stderr.read_bytes()).hexdigest(),
    }
    write_new(root / (stage + ".terminal.json"), canonical(terminal))
    print(json.dumps({"stage_uuid": stage, "exit_code": exit_code, "error_class": error_class, "no_retry": True}))
    return exit_code if exit_code is not None else 1


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as exc:
        print(json.dumps({"status": "invocation-refused", "error_class": type(exc).__name__}))
        raise SystemExit(1)
