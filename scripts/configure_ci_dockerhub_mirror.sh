#!/usr/bin/env bash
# Configure the public Docker Hub cache on GitHub-hosted Linux runners only.
set -euo pipefail

readonly MIRROR='https://mirror.gcr.io'
readonly DOCKER_RESTART_TIMEOUT='60s'
readonly DOCKER_READY_ATTEMPTS=20
readonly DOCKER_READY_PROBE_TIMEOUT='2s'
readonly DOCKER_READY_KILL_AFTER='1s'
readonly DOCKER_READY_DELAY_SECONDS=1
readonly SCRIPT_PATH="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)/$(basename -- "${BASH_SOURCE[0]}")"

hosted_linux_runner() {
  [[ "${GITHUB_ACTIONS:-}" == true &&
    "${RUNNER_ENVIRONMENT:-}" == github-hosted &&
    "${RUNNER_OS:-}" == Linux ]]
}

fail() {
  printf 'Docker Hub mirror setup failed: %s\n' "$1" >&2
  exit 1
}

docker_output() {
  # A single oversized command response is a hard failure. This keeps both
  # captured memory and diagnostic input bounded to 4 KiB per invocation.
  timeout 5s docker "$@" 2>/dev/null | head -c 4097
}

wait_for_docker_ready() {
  local attempt=0
  while (( attempt < DOCKER_READY_ATTEMPTS )); do
    attempt=$((attempt + 1))
    # Do not capture or print daemon output while startup is transient. Each
    # probe has a KILL grace, and the total configured budget is 79 seconds.
    if timeout --kill-after="$DOCKER_READY_KILL_AFTER" "$DOCKER_READY_PROBE_TIMEOUT" \
      docker info --format '{{.Driver}}' >/dev/null 2>&1; then
      return 0
    fi
    if (( attempt < DOCKER_READY_ATTEMPTS )); then
      sleep "$DOCKER_READY_DELAY_SECONDS"
    fi
  done
  return 1
}

if ! hosted_linux_runner; then
  printf 'Docker Hub mirror setup skipped outside GitHub-hosted Linux.\n'
  exit 0
fi

if [[ "${1:-}" != --as-root ]]; then
  exec sudo --preserve-env=GITHUB_ACTIONS,RUNNER_ENVIRONMENT,RUNNER_OS \
    bash "$SCRIPT_PATH" --as-root
fi
if [[ $# -ne 1 ]]; then
  fail 'unsupported arguments'
fi

command -v jq >/dev/null 2>&1 || fail 'jq is unavailable'
command -v timeout >/dev/null 2>&1 || fail 'bounded command runner is unavailable'

config_path="${SLOPSEARX_DOCKER_DAEMON_CONFIG:-/etc/docker/daemon.json}"
config_dir="$(dirname -- "$config_path")"
[[ -d "$config_dir" ]] || fail 'Docker configuration directory is unavailable'
[[ ! -L "$config_path" ]] || fail 'Docker configuration path must not be a symlink'

current_file=''
updated_file=''
cleanup() {
  [[ -z "$current_file" ]] || rm -f -- "$current_file"
  [[ -z "$updated_file" ]] || rm -f -- "$updated_file"
}
trap cleanup EXIT

current_file="$(mktemp "$config_dir/.daemon.json.current.XXXXXX")" || fail 'cannot create same-directory temporary file'
updated_file="$(mktemp "$config_dir/.daemon.json.updated.XXXXXX")" || fail 'cannot create same-directory temporary file'
if [[ -e "$config_path" ]]; then
  cat -- "$config_path" >"$current_file" || fail 'cannot read Docker configuration'
else
  : >"$current_file"
fi

# Empty files are treated as an empty object. Invalid JSON and invalid mirror
# declarations fail before any write, container inspection, or restart.
if [[ ! -s "$current_file" ]]; then
  printf '{}\n' >"$current_file"
fi
jq -e -s 'length == 1' "$current_file" >/dev/null 2>&1 || fail 'Docker configuration must contain exactly one JSON value'
jq -e --arg mirror "$MIRROR" '
  if type != "object" then error("daemon config must be an object") else . end
  | (if has("registry-mirrors") then .["registry-mirrors"] else [] end) as $mirrors
  | if ($mirrors | type) != "array" or
       any($mirrors[]; type != "string" or length == 0)
    then error("registry mirrors must be nonempty strings")
    else .["registry-mirrors"] =
      (if ($mirrors | index($mirror)) == null then $mirrors + [$mirror] else $mirrors end)
    end
' "$current_file" >"$updated_file" 2>/dev/null || fail 'Docker configuration is invalid'

if jq -S . "$current_file" 2>/dev/null | cmp -s - <(jq -S . "$updated_file"); then
  printf 'Docker Hub mirror already configured.\n'
else
  containers="$(docker_output ps --quiet)" || fail 'cannot inspect running containers'
  [[ ${#containers} -le 4096 ]] || fail 'running-container diagnostics were oversized'
  [[ -z "$containers" ]] || fail 'refusing to change Docker configuration while containers are running'

  if [[ -e "$config_path" ]]; then
    if config_mode="$(stat -c '%a' "$config_path" 2>/dev/null)"; then :
    elif config_mode="$(stat -f '%Lp' "$config_path" 2>/dev/null)"; then :
    else fail 'cannot read Docker configuration mode'
    fi
  else
    config_mode=644
  fi
  chmod "$config_mode" "$updated_file" || fail 'cannot preserve Docker configuration mode'
  mv -f -- "$updated_file" "$config_path" || fail 'cannot atomically replace Docker configuration'
  updated_file=''
  # Docker daemon restarts can exceed the five-second diagnostic command bound, especially on ARM runners.
  if timeout "$DOCKER_RESTART_TIMEOUT" systemctl restart docker >/dev/null 2>&1; then
    :
  else
    restart_status=$?
    if [[ "$restart_status" -eq 124 ]]; then
      fail 'Docker restart timed out'
    fi
    fail 'Docker restart failed'
  fi
  wait_for_docker_ready || fail 'Docker daemon readiness timed out'
  printf 'Docker Hub mirror configured.\n'
fi

# Emit only short allowlisted labels. Raw Docker output and stderr are never
# surfaced, because they may contain private registry or daemon details.
server_version="$(docker_output version --format '{{.Server.Version}}')" || fail 'Docker version diagnostics failed'
storage_driver="$(docker_output info --format '{{.Driver}}')" || fail 'Docker storage-driver diagnostics failed'
driver_status="$(docker_output info --format '{{json .DriverStatus}}')" || fail 'Docker driver-status diagnostics failed'
mirrors_json="$(docker_output info --format '{{json .RegistryConfig.Mirrors}}')" || fail 'Docker mirror diagnostics failed'
[[ ${#driver_status} -le 4096 && ${#mirrors_json} -le 4096 ]] || fail 'Docker diagnostics were oversized'
for label in "$server_version" "$storage_driver"; do
  [[ ${#label} -le 64 && "$label" =~ ^[A-Za-z0-9._+-]+$ ]] || fail 'Docker diagnostics were malformed'
done
jq -e 'type == "null" or type == "array"' >/dev/null 2>&1 <<<"${driver_status:-null}" || fail 'Docker driver diagnostics were malformed'
snapshotter="$(jq -r '
  if type == "array" then
    [ .[] | select(type == "array" and length == 2 and .[0] == "driver-type"
      and (.[1] | type == "string" and length <= 64 and test("^[A-Za-z0-9._+-]+$"))) | .[1] ][0] // ""
  else "" end
' <<<"${driver_status:-null}")" || fail 'Docker driver diagnostics were malformed'
jq -e 'type == "array" and all(.[]; type == "string")' >/dev/null 2>&1 <<<"${mirrors_json:-[]}" || fail 'Docker mirror diagnostics were malformed'
mirror_configured=false
if jq -e --arg mirror "$MIRROR" '
  any(.[]; . == $mirror or . == "https://mirror.gcr.io/"
    or . == "https://mirror.gcr.io:443" or . == "https://mirror.gcr.io:443/")
' >/dev/null 2>&1 <<<"${mirrors_json:-[]}"; then
  mirror_configured=true
fi
[[ -z "$snapshotter" ]] && snapshotter=none
printf 'Docker runtime: server=%s driver=%s containerd_snapshotter=%s public_mirror=%s\n' \
  "$server_version" "$storage_driver" "$snapshotter" "$mirror_configured"
