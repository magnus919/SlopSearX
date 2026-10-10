"""Launch the operator from source with an empty bytecode cache namespace."""

from __future__ import annotations

import os
import shutil
import sys
import tempfile
from pathlib import Path


def main() -> int:
    repository = Path(__file__).resolve().parents[1]
    cache_prefix = tempfile.mkdtemp(prefix="coverage-source-pycache-")
    os.chmod(cache_prefix, 0o700)
    sys.pycache_prefix = cache_prefix
    sys.dont_write_bytecode = True
    sys._coverage_operator_source_bootstrap = "coverage-source-bootstrap/1"
    sys.path.insert(0, str(repository))
    try:
        from scripts.coverage_operator_runner import main as run_operator

        return run_operator()
    finally:
        shutil.rmtree(cache_prefix, ignore_errors=True)


if __name__ == "__main__":  # pragma: no cover
    raise SystemExit(main())
