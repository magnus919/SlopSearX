#!/usr/bin/env bash
set -euo pipefail

if [[ $# != 2 ]]; then
  echo 'usage: rehearsal.sh PRIOR_IMAGE_ID CANDIDATE_IMAGE_ID' >&2
  exit 2
fi
fail() {
  printf '%s\n' "$1" >&2
  exit 1
}
prior=$1
candidate=$2
for image in "$prior" "$candidate"; do
  [[ $image =~ ^sha256:[a-f0-9]{64}$ ]] || fail "Expected an immutable local image ID: $image"
  [[ $(docker image inspect --format '{{.Id}}' "$image") == "$image" ]] ||
    fail "Local image identification failed: $image"
done
[[ $prior != "$candidate" ]] || fail 'Prior and candidate must have distinct image IDs'
prior_layers=$(docker image inspect --format '{{json .RootFS.Layers}}' "$prior")
candidate_layers=$(docker image inspect --format '{{json .RootFS.Layers}}' "$candidate")
[[ -n $prior_layers && $prior_layers == "$candidate_layers" ]] ||
  fail 'Candidate must preserve the inspected Python-free runtime layers'

container=
cleanup() {
  if [[ -n $container ]]; then
    docker rm --force "$container" >&2
  fi
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

activate() {
  local image=$1 port=$2
  local options=(--network none)
  if [[ -n $port ]]; then
    options+=(--env "SLOPSEARX_T02_IMAGE_PORT=$port")
  fi
  container=$(docker create --pull never --cap-drop ALL \
    --security-opt no-new-privileges --read-only --tmpfs /tmp:rw,noexec,nosuid,size=32m \
    "${options[@]}" --entrypoint /usr/local/bin/ssx-t02-http \
    "$image" /tmp/o02-rehearsal-engine-calls.jsonl)
  [[ $container =~ ^[a-f0-9]{64}$ ]] || fail 'Docker did not return an owned container ID'
  docker start "$container" >&2
  [[ $(docker inspect --format '{{.Image}} {{.State.Running}}' "$container") == "$image true" ]] ||
    fail 'Activation did not start the expected image'
}

require_health() {
  local wanted=$1 observed=0 output
  output=$(timeout 5 docker exec "$container" /usr/local/bin/ssx-healthcheck 2>&1) || observed=$?
  [[ $observed == "$wanted" && -z $output ]]
}

require_ready() {
  local attempt
  for attempt in {1..10}; do
    if require_health 0; then
      return
    fi
    sleep 1
  done
  docker logs "$container" >&2
  return 1
}

require_fixture_startup() {
  local attempt output
  for attempt in {1..10}; do
    output=$(docker logs "$container")
    if [[ $output == *'"mode":"offline-t02-fixture"'* ]]; then
      return
    fi
    sleep 1
  done
  return 1
}

retire() {
  docker stop --time 5 "$container" >&2
  [[ $(docker inspect --format '{{.State.Running}}' "$container") == false ]] ||
    fail 'Stopped container unexpectedly remained running'
  docker rm "$container" >&2
  container=
}

activate "$prior" 8080
require_ready
retire

# Keep the candidate live on an ephemeral port so the failed healthcheck is
# observed inside its running, isolated container.
activate "$candidate" ''
require_fixture_startup
require_health 1 || fail 'Candidate did not produce the expected silent health exit 1'
[[ $(docker inspect --format '{{.State.Running}}' "$container") == true ]] ||
  fail 'Candidate exited before rejection could be verified'
retire

activate "$prior" 8080
require_ready
retire

printf '{"prior_image":"%s","candidate_image":"%s","initial_health":0,"candidate_health":1,"restored_health":0,"containers_retired":true}\n' \
  "$prior" "$candidate"
