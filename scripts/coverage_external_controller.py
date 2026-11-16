"""One-shot remote receipt controller; never retries an issuer or calls providers."""

import argparse
import hashlib
import json
import os
import pathlib
import subprocess
import time

p = argparse.ArgumentParser()
p.add_argument("--config", required=True, type=pathlib.Path)
p.add_argument("--config-sha256", required=True)
p.add_argument("--source", required=True, type=pathlib.Path)
p.add_argument("--material-root", required=True, type=pathlib.Path)
p.add_argument("--output-root", required=True, type=pathlib.Path)
p.add_argument("--qualification-evidence", required=True, type=pathlib.Path)
p.add_argument("--qualification-evidence-sha256", required=True)
a = p.parse_args()
raw = a.config.read_bytes()
if hashlib.sha256(raw).hexdigest() != a.config_sha256:
    raise ValueError("config-pin-mismatch")
c = json.loads(raw)
stage = c["stage_uuid"]
root = pathlib.Path(c["directories"]["operator_handoff"]) / stage
terminal = a.output_root / (stage + ".terminal.json")
scopes = {
    "saved-corpus-import": "permit",
    "frozen-pool-collection": "permit",
    "late-registration": "registration-1",
    "protected-source-capture": "permit",
    "source-capture": "permit",
    "selector-map-admission": "map",
    "selector-operation-permits": "batch",
    "answer-execution": "permit",
}
marker = a.output_root / (stage + ".controller-started")
fd = os.open(marker, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
os.close(fd)
seen = set()
end = time.monotonic() + 28920
while time.monotonic() < end:
    if terminal.exists():
        print(json.dumps({"status": "stage-terminal", "stage_uuid": stage}), flush=True)
        raise SystemExit(0)
    for scope, request_id in scopes.items():
        if scope in seen or not (root / scope / "requests" / (request_id + ".json")).exists():
            continue
        seen.add(scope)
        command = [
            str(a.source / ".venv/bin/python"),
            "-I",
            str(a.material_root / "coverage_external_issuer.py"),
            "--source",
            str(a.source),
            "--source-revision",
            c["source_revision"],
            "--config",
            str(a.config),
            "--config-sha256",
            a.config_sha256,
            "--scope",
            scope,
            "--request-id",
            request_id,
        ]
        if scope not in {"saved-corpus-import", "frozen-pool-collection", "late-registration"}:
            active = root / "late-registration/responses/registration-1.receipt"
            pin = (root / "late-registration/pins/registration-1.sha256").read_text().strip()
            command += ["--active-registration", str(active), "--active-registration-sha256", pin]
        if scope == "protected-source-capture":
            proof = a.qualification_evidence
            command += [
                "--qualification-evidence",
                str(proof),
                "--qualification-evidence-sha256",
                a.qualification_evidence_sha256,
            ]
        result = subprocess.run(command, cwd=a.source, capture_output=True)
        for label, data in [("stdout", result.stdout), ("stderr", result.stderr)]:
            path = a.output_root / (stage + ".issuer-" + scope + "." + label)
            fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
            with os.fdopen(fd, "wb") as f:
                f.write(data)
                f.flush()
                os.fsync(f.fileno())
        print(json.dumps({"scope": scope, "exit_code": result.returncode, "retry": False}), flush=True)
        if result.returncode:
            failure = a.output_root / (stage + ".controller-failure.json")
            fd = os.open(failure, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
            with os.fdopen(fd, "w") as f:
                json.dump({"scope": scope, "exit_code": result.returncode, "retry": False}, f, sort_keys=True)
                f.flush()
                os.fsync(f.fileno())
            raise SystemExit(result.returncode)
    time.sleep(2)
raise SystemExit("controller-deadline-exceeded")
