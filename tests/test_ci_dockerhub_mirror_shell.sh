#!/usr/bin/env bash
# Deterministic integration checks for the shipped shell helper.
set -euo pipefail

root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
helper="$root/scripts/configure_ci_dockerhub_mirror.sh"
base="${SLOPSEARX_TEST_TEMP_ROOT:-${RUNNER_TEMP:-}}"
[[ -n "$base" && -d "$base" ]] || { printf 'SLOPSEARX_TEST_TEMP_ROOT or RUNNER_TEMP must name an existing directory\n' >&2; exit 2; }
work="$(mktemp -d "$base/o02-mirror-test.XXXXXX")"
cleanup() { rm -rf -- "$work"; }
trap cleanup EXIT

mkdir -p "$work/bin" "$work/etc/docker"
cat >"$work/bin/timeout" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
[[ "$1" == 5s ]] || exit 90
shift
exec "$@"
EOF
cat >"$work/bin/docker" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
printf '%s\n' "$*" >>"$DOCKER_LOG"
case "$*" in
  'ps --quiet')
    if [[ "${DOCKER_PS_FAIL:-}" == true ]]; then printf 'private-daemon-detail\n' >&2; exit 12; fi
    printf '%s\n' "${DOCKER_CONTAINERS:-}"
    ;;
  'version --format {{.Server.Version}}') printf '%s\n' '28.0.4' ;;
  'info --format {{.Driver}}') printf '%s\n' 'overlay2' ;;
  'info --format {{json .DriverStatus}}') printf '%s\n' "${DOCKER_DRIVER_STATUS:-[[\"driver-type\",\"io.containerd.snapshotter.v1\"]]}" ;;
  'info --format {{json .RegistryConfig.Mirrors}}') printf '%s\n' "${DOCKER_MIRRORS:-[\"https://mirror.gcr.io\"]}" ;;
  *) exit 91 ;;
esac
EOF
cat >"$work/bin/systemctl" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
printf '%s\n' "$*" >>"$SYSTEMCTL_LOG"
if [[ "${SYSTEMCTL_FAIL:-}" == true ]]; then printf 'private-service-detail\n' >&2; exit 13; fi
[[ "$*" == 'restart docker' ]]
EOF
chmod +x "$work/bin/timeout" "$work/bin/docker" "$work/bin/systemctl"
export PATH="$work/bin:$PATH"
export DOCKER_LOG="$work/docker.log" SYSTEMCTL_LOG="$work/systemctl.log"
export GITHUB_ACTIONS=true RUNNER_ENVIRONMENT=github-hosted RUNNER_OS=Linux
export SLOPSEARX_DOCKER_DAEMON_CONFIG="$work/etc/docker/daemon.json"
: >"$DOCKER_LOG"
: >"$SYSTEMCTL_LOG"

# Non-hosted invocations must not touch Docker or the configured path.
if GITHUB_ACTIONS=true RUNNER_ENVIRONMENT=self-hosted RUNNER_OS=Linux bash "$helper" >"$work/skip.out"; then :; else exit 1; fi
[[ ! -e "$SLOPSEARX_DOCKER_DAEMON_CONFIG" && ! -s "$DOCKER_LOG" && ! -s "$SYSTEMCTL_LOG" ]]

printf '{"debug":true,"insecure-registries":["registry.example:5000"],"registry-mirrors":["https://existing.example"]}\n' >"$SLOPSEARX_DOCKER_DAEMON_CONFIG"
chmod 640 "$SLOPSEARX_DOCKER_DAEMON_CONFIG"
bash "$helper" --as-root >"$work/configured.out"
jq -e '.debug == true and .["insecure-registries"] == ["registry.example:5000"] and .["registry-mirrors"] == ["https://existing.example", "https://mirror.gcr.io"]' "$SLOPSEARX_DOCKER_DAEMON_CONFIG" >/dev/null
[[ "$(stat -c '%a' "$SLOPSEARX_DOCKER_DAEMON_CONFIG" 2>/dev/null || stat -f '%Lp' "$SLOPSEARX_DOCKER_DAEMON_CONFIG")" == 640 ]]
[[ "$(cat "$SYSTEMCTL_LOG")" == 'restart docker' ]]
[[ "$(grep -c '^ps --quiet$' "$DOCKER_LOG")" == 1 ]]
grep -q 'containerd_snapshotter=io.containerd.snapshotter.v1 public_mirror=true' "$work/configured.out"

# Idempotence leaves daemon bytes and restart count unchanged.
before="$(shasum -a 256 "$SLOPSEARX_DOCKER_DAEMON_CONFIG" | awk '{print $1}')"
bash "$helper" --as-root >"$work/idempotent.out"
after="$(shasum -a 256 "$SLOPSEARX_DOCKER_DAEMON_CONFIG" | awk '{print $1}')"
[[ "$before" == "$after" && "$(wc -l <"$SYSTEMCTL_LOG" | tr -d ' ')" == 1 ]]

# Running containers and malformed config fail closed without mutation/restart.
printf '{"debug":true}\n' >"$SLOPSEARX_DOCKER_DAEMON_CONFIG"
cp "$SLOPSEARX_DOCKER_DAEMON_CONFIG" "$work/original.json"
if DOCKER_CONTAINERS=container123 bash "$helper" --as-root >"$work/running.out" 2>"$work/running.err"; then exit 1; fi
cmp -s "$work/original.json" "$SLOPSEARX_DOCKER_DAEMON_CONFIG"
grep -q 'containers are running' "$work/running.err"
! grep -q container123 "$work/running.err"
[[ "$(wc -l <"$SYSTEMCTL_LOG" | tr -d ' ')" == 1 ]]

# Malformed registry-mirror settings fail closed without changing the file.
printf '{"registry-mirrors":"private-value"}\n' >"$SLOPSEARX_DOCKER_DAEMON_CONFIG"
cp "$SLOPSEARX_DOCKER_DAEMON_CONFIG" "$work/bad-mirrors-original.json"
if bash "$helper" --as-root >"$work/bad-mirrors.out" 2>"$work/bad-mirrors.err"; then exit 1; fi
cmp -s "$work/bad-mirrors-original.json" "$SLOPSEARX_DOCKER_DAEMON_CONFIG"
! grep -q 'private-value' "$work/bad-mirrors.err"

printf '{not-json}\n' >"$SLOPSEARX_DOCKER_DAEMON_CONFIG"
cp "$SLOPSEARX_DOCKER_DAEMON_CONFIG" "$work/invalid.json"
if bash "$helper" --as-root >"$work/invalid.out" 2>"$work/invalid.err"; then exit 1; fi
cmp -s "$work/invalid.json" "$SLOPSEARX_DOCKER_DAEMON_CONFIG"
! grep -q 'not-json' "$work/invalid.err"
[[ "$(wc -l <"$SYSTEMCTL_LOG" | tr -d ' ')" == 1 ]]

# jq can parse a stream of documents, but daemon.json must contain one value.
printf '{"debug":true}\n{"debug":false}\n' >"$SLOPSEARX_DOCKER_DAEMON_CONFIG"
cp "$SLOPSEARX_DOCKER_DAEMON_CONFIG" "$work/multiple-documents-original.json"
if bash "$helper" --as-root >"$work/multiple-documents.out" 2>"$work/multiple-documents.err"; then exit 1; fi
cmp -s "$work/multiple-documents-original.json" "$SLOPSEARX_DOCKER_DAEMON_CONFIG"
grep -q 'exactly one JSON value' "$work/multiple-documents.err"
[[ "$(wc -l <"$SYSTEMCTL_LOG" | tr -d ' ')" == 1 ]]

# Docker inspection failures do not mutate the file or expose command output.
printf '{"debug":false}\n' >"$SLOPSEARX_DOCKER_DAEMON_CONFIG"
cp "$SLOPSEARX_DOCKER_DAEMON_CONFIG" "$work/docker-failure-original.json"
if DOCKER_PS_FAIL=true bash "$helper" --as-root >"$work/docker-failure.out" 2>"$work/docker-failure.err"; then exit 1; fi
cmp -s "$work/docker-failure-original.json" "$SLOPSEARX_DOCKER_DAEMON_CONFIG"
grep -q 'cannot inspect running containers' "$work/docker-failure.err"
! grep -q 'private-daemon-detail' "$work/docker-failure.err"
[[ "$(wc -l <"$SYSTEMCTL_LOG" | tr -d ' ')" == 1 ]]

# A failed restart is surfaced as failure without leaking systemctl output.
if SYSTEMCTL_FAIL=true bash "$helper" --as-root >"$work/restart-failure.out" 2>"$work/restart-failure.err"; then exit 1; fi
jq -e '.["registry-mirrors"] == ["https://mirror.gcr.io"]' "$SLOPSEARX_DOCKER_DAEMON_CONFIG" >/dev/null
grep -q 'Docker restart failed' "$work/restart-failure.err"
! grep -q 'private-service-detail' "$work/restart-failure.err"
[[ "$(wc -l <"$SYSTEMCTL_LOG" | tr -d ' ')" == 2 ]]

# Diagnostics accept the same safe public endpoint spellings as the old helper
# and reject credentialed, noncanonical mirrors without exposing their secrets.
printf '{"registry-mirrors":["https://mirror.gcr.io:443/"]}\n' >"$SLOPSEARX_DOCKER_DAEMON_CONFIG"
if DOCKER_MIRRORS='["https://mirror.gcr.io/"]' bash "$helper" --as-root >"$work/canonical.out"; then :; else exit 1; fi
grep -q 'public_mirror=true' "$work/canonical.out"
if DOCKER_MIRRORS='["https://user:private@mirror.gcr.io/cache"]' bash "$helper" --as-root >"$work/noncanonical.out"; then :; else exit 1; fi
grep -q 'public_mirror=false' "$work/noncanonical.out"
! grep -q 'private@' "$work/noncanonical.out"

printf 'native Docker Hub mirror shell checks passed\n'
