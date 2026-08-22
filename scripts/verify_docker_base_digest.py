#!/usr/bin/env python3
"""Verify that Dockerfile's pinned Python digest matches Docker Hub."""

import argparse
import json
import re
import sys
import time
import urllib.parse
import urllib.request
from pathlib import Path
from urllib.error import HTTPError, URLError

FROM_RE = re.compile(
    r"^\s*FROM\s+python:(?P<tag>[^@\s]+)@(?P<digest>sha256:[0-9a-f]{64})(?:\s+AS\s+\S+)?\s*$",
    re.IGNORECASE | re.MULTILINE,
)
ALL_FROM_RE = re.compile(r"^\s*FROM\b.*$", re.IGNORECASE | re.MULTILINE)
DIGEST_RE = re.compile(r"^sha256:[0-9a-f]{64}$")
MAX_ATTEMPTS = 3
RETRYABLE_HTTP_STATUS = frozenset({429, 500, 502, 503, 504})
RETRY_DELAY_SECONDS = 1


class RegistryLookupError(RuntimeError):
    """The registry could not provide a trustworthy digest."""


def _urlopen_with_retries(request: str | urllib.request.Request, *, label: str):
    last_error: Exception | None = None
    detail = "unknown error"
    attempts = 0

    for attempt in range(MAX_ATTEMPTS):
        attempts = attempt + 1
        try:
            return urllib.request.urlopen(request, timeout=30)
        except HTTPError as exc:
            last_error = exc
            detail = f"HTTP {exc.code}"
            retryable = exc.code in RETRYABLE_HTTP_STATUS
        except (URLError, TimeoutError) as exc:
            last_error = exc
            detail = str(getattr(exc, "reason", exc))
            retryable = True

        if not retryable or attempt == MAX_ATTEMPTS - 1:
            break
        time.sleep(RETRY_DELAY_SECONDS * (2**attempt))

    raise RegistryLookupError(f"{label} failed with {detail} after {attempts} attempt(s)") from last_error


def _pinned_python_base(dockerfile: str) -> tuple[str, str]:
    from_lines = list(ALL_FROM_RE.finditer(dockerfile))
    if len(from_lines) != 1:
        raise SystemExit("Dockerfile must contain exactly one FROM line for base verification")

    match = FROM_RE.fullmatch(from_lines[0].group(0))
    if match is None:
        raise SystemExit("Dockerfile must pin python base image by tag and digest")
    return match.group("tag"), match.group("digest")


def registry_digest(repository: str, tag: str) -> str:
    scope = urllib.parse.quote(f"repository:{repository}:pull", safe="")
    token_url = f"https://auth.docker.io/token?service=registry.docker.io&scope={scope}"
    try:
        with _urlopen_with_retries(token_url, label="Docker Hub token request") as response:
            token_payload = json.load(response)
    except (json.JSONDecodeError, KeyError, TypeError) as exc:
        raise RegistryLookupError("Docker Hub token response did not contain a token") from exc

    if not isinstance(token_payload, dict):
        raise RegistryLookupError("Docker Hub token response did not contain a token")
    token = token_payload.get("token")
    if not isinstance(token, str) or not token:
        raise RegistryLookupError("Docker Hub token response did not contain a token")

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
    with _urlopen_with_retries(request, label=f"Docker Hub manifest request for python:{tag}") as response:
        digest = response.headers.get("Docker-Content-Digest", "")
    if not DIGEST_RE.fullmatch(digest):
        raise RegistryLookupError("Docker Hub did not return a valid manifest digest")
    return digest


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--dockerfile", default="Dockerfile")
    args = parser.parse_args()

    try:
        tag, expected = _pinned_python_base(Path(args.dockerfile).read_text())
        actual = registry_digest("library/python", tag)
    except OSError as exc:
        print(f"Unable to read Dockerfile: {exc}", file=sys.stderr)
        return 2
    except RegistryLookupError as exc:
        print(f"Docker Hub registry lookup failed: {exc}", file=sys.stderr)
        return 2

    print(f"python:{tag} expected={expected} registry={actual}")
    if expected != actual:
        print("Pinned Docker base digest is stale; update tag and digest together.", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
