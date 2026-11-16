"""Current write-once entrypoint for the frozen temporary T06 G08 producers.

Historical producer bytes stay intact. Never invoke them directly for a new
observation: this wrapper checks the destination before importing any backend,
checks source/interpreter/SDK without assert, disables bytecode, and publishes
through exclusive creation. It is a manual oracle, never a native gate.
"""

import argparse
import hashlib
import importlib.metadata
import json
import os
import runpy
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

PRODUCERS = (
    "capture.py",
    "legacy_capture.py",
    "policy_capture.py",
    "boundaries_capture.py",
    "entities_capture.py",
    "snapshots_capture.py",
    "prompts_capture.py",
    "bounds_capture.py",
    "prompts_raw_capture.py",
    "prompts_unknown_capture.py",
    "prompts_envelope_capture.py",
)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("producer", choices=PRODUCERS)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    # This check deliberately precedes metadata, backend and guard imports.
    if os.path.lexists(args.output):
        raise RuntimeError("refusing existing observation output")
    if not args.output.parent.is_dir():
        raise RuntimeError("output parent must already exist")
    sys.dont_write_bytecode = True
    if sys.version_info[:3] != (3, 14, 7):
        raise RuntimeError("requires pinned CPython 3.14.7")
    versions = {k: importlib.metadata.version(k) for k in ("fastmcp", "mcp")}
    if versions != {"fastmcp": "4.0.11", "mcp": "2.3.0"}:
        raise RuntimeError("wrong reference SDKs")
    directory = Path(__file__).resolve().parent
    root = directory.parents[2]
    bindings = json.loads((directory / "source_bindings_v2.json").read_text())
    git_env = {k: v for k, v in os.environ.items() if k in ("PATH", "HOME", "TMPDIR")}
    head = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root, env=git_env, text=True).strip()
    if head != bindings["baseline"]:
        raise RuntimeError("wrong historical source revision")
    for row in bindings["sources"] + bindings["artifacts"]:
        if hashlib.sha256((root / row["file"]).read_bytes()).hexdigest() != row["sha256"]:
            raise RuntimeError("source or producer/capture binding drift: " + row["file"])
    # The original producer installs the existing sanitized-environment,
    # shared-store rejection and loopback-only network guards before imports.
    # Its write target is now always a newly created exclusive private directory.
    with tempfile.TemporaryDirectory(prefix="t06-observation-", dir=args.output.parent) as tmp:
        staged = Path(tmp) / "observation.json"
        sys.argv = [str(directory / args.producer), str(staged)]
        runpy.run_path(directory / args.producer, run_name="__main__")
        if not staged.is_file():
            raise RuntimeError("producer did not produce an observation")
        # Exclusive publication also closes the check-to-write race. Existing
        # observations cannot be overwritten by the original write_text call.
        descriptor = os.open(args.output, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
        with os.fdopen(descriptor, "wb") as output, staged.open("rb") as source:
            shutil.copyfileobj(source, output, length=65536)
            output.flush()
            os.fsync(output.fileno())


if __name__ == "__main__":
    main()
