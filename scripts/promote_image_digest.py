#!/usr/bin/env python3
"""Promote the pinned application-image digest when CI publishes a newer one.

The deployment surfaces (docker-compose.yml, k8s/deployment.yaml) and their
tests pin ``ghcr.io/magnus919/slopsearx`` by immutable digest so deployments
always run the exact artifact CI built, scanned, and smoke-tested. When a new
main build republishes ``latest``, this script detects the drift and rewrites
the pin everywhere, so a scheduled workflow can open the promotion PR.

Modes:
  --check   Compare only. Exit 0 when current, 1 when drifted, 2 on lookup
            or pin-state errors. Never writes.
  --write   Rewrite the pinned digest in place. Exit 0 when already current,
            10 when files were updated, 2 on errors.

Registry lookups are anonymous (the package is public) and retried with
backoff, mirroring scripts/verify_docker_base_digest.py.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
import time
import urllib.request

IMAGE = "ghcr.io/magnus919/slopsearx"
DIGEST_RE = re.compile(rf"{re.escape(IMAGE)}@sha256:[0-9a-f]{{64}}")

# Files whose combined contents must carry exactly one pinned digest.
PIN_FILES = (
    "docker-compose.yml",
    "k8s/deployment.yaml",
    "tests/test_deployment_artifacts.py",
)

REGISTRY_TOKEN_URL = f"https://ghcr.io/token?scope=repository:{IMAGE.removeprefix('ghcr.io/')}:pull"
REGISTRY_MANIFEST_URL = f"https://ghcr.io/v2/{IMAGE.removeprefix('ghcr.io/')}/manifests/latest"
ACCEPT_MANIFEST_TYPES = ", ".join(
    [
        "application/vnd.oci.image.index.v1+json",
        "application/vnd.docker.distribution.manifest.list.v2+json",
        "application/vnd.oci.image.manifest.v1+json",
        "application/vnd.docker.distribution.manifest.v2+json",
    ]
)

MAX_ATTEMPTS = 3


class PromotionError(RuntimeError):
    """Pin-state or registry failure that must block promotion."""


def _urlopen_with_retries(request: urllib.request.Request, *, label: str) -> object:
    last_error: Exception | None = None
    for attempt in range(1, MAX_ATTEMPTS + 1):
        try:
            return urllib.request.urlopen(request, timeout=30)
        except Exception as exc:  # noqa: BLE001 - classified below
            last_error = exc
            if attempt < MAX_ATTEMPTS:
                time.sleep(2**attempt)
    raise PromotionError(f"{label} failed after {MAX_ATTEMPTS} attempts: {last_error}")


def resolve_latest_digest() -> str:
    """Return the digest 'latest' currently points at, as sha256:..."""
    try:
        with _urlopen_with_retries(urllib.request.Request(REGISTRY_TOKEN_URL), label="token request") as resp:
            body = json.load(resp)  # type: ignore[attr-defined]
        token = body["token"]
    except Exception as exc:  # noqa: BLE001
        raise PromotionError(f"could not obtain registry token: {exc}") from exc

    request = urllib.request.Request(
        REGISTRY_MANIFEST_URL,
        headers={"Authorization": f"Bearer {token}", "Accept": ACCEPT_MANIFEST_TYPES},
    )
    try:
        with _urlopen_with_retries(request, label="manifest request") as resp:  # type: ignore[arg-type]
            digest = resp.headers.get("Docker-Content-Digest", "")  # type: ignore[attr-defined]
    except PromotionError:
        raise
    except Exception as exc:  # noqa: BLE001
        raise PromotionError(f"could not resolve latest manifest: {exc}") from exc

    if not digest.startswith("sha256:"):
        raise PromotionError(f"registry returned no usable digest: {digest!r}")
    return digest


def extract_pinned_digest(root_path) -> str:
    """Collect the pinned digest across PIN_FILES; fail closed on ambiguity."""
    import pathlib

    pins: set[str] = set()
    for name in PIN_FILES:
        path = pathlib.Path(root_path) / name
        try:
            text = path.read_text()
        except FileNotFoundError as exc:
            raise PromotionError(f"missing expected pin file: {name}") from exc
        pins.update(DIGEST_RE.findall(text))
    if len(pins) != 1:
        raise PromotionError(f"expected exactly one unique pinned digest, found {sorted(pins)}")
    # Return the bare digest (sha256:...) so callers can compare directly
    # against registry output; apply_promotion rebuilds the full ref.
    return pins.pop().rsplit("@", 1)[1]


def apply_promotion(root_path, new_digest: str) -> list[str]:
    """Rewrite every pinned reference to new_digest; return changed files."""
    import pathlib

    replacement = f"{IMAGE}@{new_digest}"
    changed: list[str] = []
    for name in PIN_FILES:
        path = pathlib.Path(root_path) / name
        text = path.read_text()
        updated = DIGEST_RE.sub(replacement, text)
        if updated != text:
            path.write_text(updated)
            changed.append(name)
    return changed


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="report drift without writing")
    parser.add_argument("--root", default=".", help="repository root (default: cwd)")
    args = parser.parse_args(argv)

    try:
        pinned = extract_pinned_digest(args.root)
        latest = resolve_latest_digest()
    except PromotionError as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 2

    if pinned == latest:
        print(f"{IMAGE} pin is current: {pinned}")
        return 0

    print(f"{IMAGE} drift detected:\n  pinned: {pinned}\n  latest: {latest}")
    if args.check:
        return 1

    changed = apply_promotion(args.root, latest)
    print(f"updated pin in: {', '.join(changed)}")
    return 10


if __name__ == "__main__":
    raise SystemExit(main())
