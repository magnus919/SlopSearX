# Python dependency audit during the Rust migration

This audit keeps the retained Python compatibility environment usable while the
Rust migration proceeds. It does not change the final requirement to remove the
Python backend. See [the rewrite case study](CASE_STUDY.md), [issue
#1129](https://github.com/magnus919/SlopSearX/issues/1129), and [the source
successor capsule](t06/source-amendments/python-dependency-audit-current-main-v1/capsule.json).

The CI quality job installs the project and its development extra from
`uv.lock`, then audits two scopes:

- The installed audit covers the resolved runtime dependencies and development
  tools, including the scanner, pip, and setuptools in that environment.
- The runtime audit covers the version-pinned and hash-pinned runtime dependency
  closure exported from the same lock, with no development extra requested.
  `--no-deps --disable-pip` prevents the scanner from resolving a different graph.

The unpublished local project is the only editable distribution in this locked
graph. `--skip-editable` excludes that project from the installed audit because
there is no corresponding published PyPI artifact to query; it does not exempt
its installed dependencies. The runtime export excludes the project explicitly
with `--no-emit-project`. If another editable dependency is introduced, review
this assumption and the audit scope before accepting the lock change.

## Reproduce the CI scopes

Use CPython 3.12 and the bootstrap uv version selected by the CI workflow. Keep
the environment, cache, and report directories in an owned persistent build
directory for local work; the example uses a designated CI job directory.

```sh
python -m pip install "uv==0.11.18"
export UV_PROJECT_ENVIRONMENT="$RUNNER_TEMP/python-dependency-audit-venv"
uv sync --locked --extra dev --python "$(command -v python)"
export PATH="$UV_PROJECT_ENVIRONMENT/bin:$PATH"

audit_dir="$RUNNER_TEMP/python-dependency-audit"
mkdir -p "$audit_dir"
uv --version | tee "$audit_dir/uv-version.txt"
python --version | tee "$audit_dir/python-version.txt"
python -m pip_audit --version | tee "$audit_dir/pip-audit-version.txt"
printf 'PyPI JSON vulnerability service: https://pypi.org/pypi/<package>/<version>/json\n' \
  > "$audit_dir/vulnerability-service.txt"

uv lock --check
python -m pip check
python -m pip list --format=json > "$audit_dir/installed-packages.json"
python -m pip_audit --local --skip-editable --vulnerability-service pypi \
  --format json --output "$audit_dir/installed.json"
uv export --locked --no-dev --no-emit-project --format requirements-txt \
  --output-file "$audit_dir/runtime-requirements.txt"
python -m pip_audit --requirement "$audit_dir/runtime-requirements.txt" \
  --no-deps --disable-pip --strict --vulnerability-service pypi \
  --format json --output "$audit_dir/runtime.json"
```

The current lock pins pip-audit **2.10.1**. CI records the actual scanner,
interpreter, and uv identities and queries the explicit **PyPI** vulnerability
service. That service is mutable and does not expose an immutable database
revision; reports are observations at execution time. CI uploads the report
directory as the `python-dependency-audit` artifact, including on failure, with
14-day retention. Preserve required release or investigation evidence separately
before artifact expiration.

The installed audit intentionally does not use pip-audit `--strict`, because the
unpublished editable `slopsearx` distribution is the one allowed skip. The CI
step compares normalized package names from `pip list` with the audit report,
requires exactly that one skip with reason `distribution marked as editable`,
and requires every other report entry to contain a version and an empty
vulnerability list. The runtime audit uses `--strict` and its report must have
no skips or findings. A successful scanner exit by itself is not sufficient
when a dependency was skipped.

## Maintain the gate

Do not add `--ignore-vuln` flags, blanket scanner exceptions, or advisory
suppression lists. Update affected package floors and the lock, inspect the
resolved change set, and rerun both audit scopes. A failure must remain visible.
Keep the existing requirements-file pip-audit scan and the CRITICAL/HIGH image
Trivy gates active; these scans cover different inputs.

Preserve historical lock snapshots, source hashes, and qualification receipts.
The separate named #1129 source readers authenticate current physical material,
then reconstruct the accepted Git predecessors using the bounded capsule.
Historical observations retain their original dependency profiles. A clean
audit does not qualify all MCP, portal, platform, deployment, or migration
behavior, and an advisory does not establish that SlopSearX executes the
vulnerable code path or is exploitable.

## OAuth warning and the retained compatibility boundary

The successful full Python candidate run on November 15, 2026 (3,241 passed,
61 skipped, 51 subtests passed) emitted the MCP warning that
`AuthSettings.validate_token_resource` is unset. The warning remains in the raw
test output. Current Python configuration leaves that setting unset, and
`SlopSearxOAuthProvider._issue_tokens` drops the supplied resource when issuing
tokens. This is a pre-existing legacy limitation documented by
[T04's intentional resource-retention correction](t04/README.md#source-parity-and-intentional-corrections)
and [T05's foreign-resource correction](t05/PARITY.md), rather than a dependency
upgrade regression.

Rust retains supplied resource bindings through issuance and refresh and checks
them at authorization, exchange, and bearer admission. An explicitly foreign
resource is rejected. Omitted resource remains intentionally supported; this
correction does not impose a new mandatory-resource policy on existing clients.

Setting the Python flag alone would not establish enforcement on the current
modern provider middleware path. That path constructs its bearer backend
separately, and a complete correction would require verified binding retention
and middleware wiring as well as behavioral evidence. This dependency refresh
does not change that Python behavior or suppress its warning. The clean audit
and passing compatibility suite do not certify resource-audience enforcement
beyond the documented boundary.
