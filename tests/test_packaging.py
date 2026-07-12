"""Tests for production package metadata."""

import tomllib
from pathlib import Path


def test_cssselect_is_a_runtime_dependency() -> None:
    """HTML adapters use lxml.cssselect(), which needs cssselect installed."""
    pyproject_path = Path(__file__).parents[1] / "pyproject.toml"
    pyproject = tomllib.loads(pyproject_path.read_text())

    dependencies = pyproject["project"]["dependencies"]
    assert any(dependency.startswith("cssselect") for dependency in dependencies)
