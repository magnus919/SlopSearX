#!/usr/bin/env python3
"""Verify that Dockerfile's pinned Python digest matches Docker Hub."""

import argparse
import json
import re
import sys
import urllib.parse
import urllib.request
from pathlib import Path

FROM_RE = re.compile(
    r"^FROM\s+python:(?P<tag>[^@\s]+)@(?P<digest>sha256:[0-9a-f]{64})\s*$",
    re.MULTILINE,
)
DIGEST_RE = re.compile(r"^sha256:[0-9a-f]{64}$")


def registry_digest(repository: str, tag: str) -> str:
    scope = urllib.parse.quote(f"repository:{repository}:pull", safe="")
    token_url = f"https://auth.docker.io/token?service=registry.docker.io&scope={scope}"
    with urllib.request.urlopen(token_url, timeout=30) as response:
        token = json.load(response)["token"]

    request = urllib.request.Request(
        f"https://registry-1.docker.io/v2/{repository}/manifests/{tag}",
        headers={
            "Authorization": f"Bearer {token}",
            "Accept": (
                "application/vnd.oci.image.index.v1+json, "
                "application/vnd.docker.distribution.manifest.list.v2+json, "
                "application/vnd.docker.distribution.manifest.v2+json"
            ),
        },
    )
    with urllib.request.urlopen(request, timeout=30) as response:
        digest = response.headers.get("Docker-Content-Digest", "")
    if not DIGEST_RE.fullmatch(digest):
        raise RuntimeError("Docker Hub did not return a valid manifest digest")
    return digest


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--dockerfile", default="Dockerfile")
    args = parser.parse_args()

    match = FROM_RE.search(Path(args.dockerfile).read_text())
    if match is None:
        raise SystemExit("Dockerfile must pin python base image by tag and digest")

    tag = match.group("tag")
    expected = match.group("digest")
    actual = registry_digest("library/python", tag)
    print(f"python:{tag} expected={expected} registry={actual}")
    if expected != actual:
        print("Pinned Docker base digest is stale; update tag and digest together.", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
