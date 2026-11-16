#!/bin/sh
# Exercise the compiled foundation server without Python and inspect its native
# artifact/dependency boundary. This is foundation readiness evidence only.
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
server_bin=${SSX_SERVER_BIN:-"$repo_root/target/debug/ssx-server"}
lockfile=${CARGO_LOCKFILE:-"$repo_root/Cargo.lock"}

fail() {
    printf 'rust foundation smoke: %s\n' "$*" >&2
    exit 1
}

[ -x "$server_bin" ] || fail "compiled server is missing or not executable: $server_bin"
[ -f "$lockfile" ] || fail "Cargo.lock is missing: $lockfile"
command -v ps >/dev/null 2>&1 || fail "ps is required for bounded server shutdown"
scratch=$(mktemp -d "${TMPDIR:-/tmp}/ssx-foundation.XXXXXX")
server_pid=
cleanup() {
    if [ -n "$server_pid" ] && kill -0 "$server_pid" 2>/dev/null; then
        kill -TERM "$server_pid" 2>/dev/null || true
        attempts=0
        while server_alive && [ "$attempts" -lt 20 ]; do
            attempts=$((attempts + 1))
            sleep 0.1
        done
        if server_alive; then
            kill -KILL "$server_pid" 2>/dev/null || true
        fi
        wait "$server_pid" 2>/dev/null || true
    fi
    rm -rf "$scratch"
}
trap cleanup EXIT
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM
server_alive() {
    command -v ps >/dev/null 2>&1 || return 1
    process_state=$(ps -p "$server_pid" -o stat= 2>/dev/null || true)
    case "$process_state" in
        ''|Z*) return 1 ;;
        *) return 0 ;;
    esac
}

# The final runtime may not embed CPython or depend on a Python bridge crate.
if grep -Eq '^name = "(cpython|pyo3|pyo3-ffi|pyo3-macros|pythonize|rust-cpython)"$' "$lockfile"; then
    fail "Cargo.lock contains a Python bridge dependency"
fi
host_os=$(uname -s)
case "$host_os" in
    Linux)
        command -v readelf >/dev/null 2>&1 || fail "readelf is required for the Linux artifact audit"
        command -v nm >/dev/null 2>&1 || fail "nm is required for the Linux artifact audit"
        readelf -d "$server_bin" >"$scratch/readelf" 2>&1 \
            || fail "readelf could not inspect the server executable"
        if grep -Eiq 'libpython([0-9._-]*)' "$scratch/readelf"; then
            fail "server dynamically links libpython"
        fi
        nm -D "$server_bin" >"$scratch/nm" 2>&1 \
            || fail "nm could not inspect dynamic server symbols"
        if grep -Eq '(^|[[:space:]])_{0,2}Py(_|[A-Z])[[:alnum:]_]*(@|$|[[:space:]])' "$scratch/nm"; then
            fail "server imports CPython symbols"
        fi
        ;;
    Darwin)
        command -v otool >/dev/null 2>&1 || fail "otool is required for the macOS artifact audit"
        command -v nm >/dev/null 2>&1 || fail "nm is required for the macOS artifact audit"
        otool -L "$server_bin" >"$scratch/otool" 2>&1 \
            || fail "otool could not inspect the server executable"
        if grep -Eiq 'libpython([0-9._-]*)' "$scratch/otool"; then
            fail "server dynamically links libpython"
        fi
        nm -g -u "$server_bin" >"$scratch/nm" 2>&1 \
            || fail "nm could not inspect undefined server symbols"
        if grep -Eq '(^|[[:space:]])_{0,2}Py(_|[A-Z])[[:alnum:]_]*(@|$|[[:space:]])' "$scratch/nm"; then
            fail "server imports CPython symbols"
        fi
        ;;
    *)
        fail "artifact dependency audit is not implemented for $host_os"
        ;;
esac

# These commands are tripwires, not interpreters. The server receives no usable
# Python executable on PATH; attempting `python` or `python3` leaves a marker.
mkdir "$scratch/path"
for command_name in python python3; do
    cat >"$scratch/path/$command_name" <<'SENTINEL'
#!/bin/sh
printf '%s\n' "${0##*/}" >>"$SSX_PYTHON_SENTINEL"
exit 97
SENTINEL
    chmod 700 "$scratch/path/$command_name"
done

/bin/sh -c 'trap - INT; exec /usr/bin/env PATH="$1" SSX_PYTHON_SENTINEL="$2" "$3" --listen 127.0.0.1:0' \
    smoke-server "$scratch/path" "$scratch/python-called" "$server_bin" \
    >"$scratch/server.log" 2>&1 &
server_pid=$!

address=
attempt=0
while [ "$attempt" -lt 100 ]; do
    address=$(sed -n 's/.*"address":"\([^"]*\)".*/\1/p' "$scratch/server.log" | head -n 1)
    if [ -n "$address" ]; then
        break
    fi
    if ! server_alive; then
        cat "$scratch/server.log" >&2
        fail "server exited before publishing its listen address"
    fi
    attempt=$((attempt + 1))
    sleep 0.1
done
[ -n "$address" ] || { cat "$scratch/server.log" >&2; fail "server did not become ready"; }
grep -Eq '"mode":"foundation"' "$scratch/server.log" || fail "server did not identify foundation mode"

http_code=
attempt=0
while [ "$attempt" -lt 100 ]; do
    http_code=$(curl --silent --output "$scratch/health.json" --write-out '%{http_code}' \
        --max-time 1 "http://$address/healthz" 2>/dev/null || true)
    [ "$http_code" = 200 ] && break
    if ! server_alive; then
        cat "$scratch/server.log" >&2
        fail "server exited before returning readiness"
    fi
    attempt=$((attempt + 1))
    sleep 0.1
done
[ "$http_code" = 200 ] || fail "GET /healthz returned HTTP ${http_code:-no response}"
grep -Eq '^\{"status":"ok","mode":"foundation"\}$' "$scratch/health.json" \
    || fail "GET /healthz returned an unexpected readiness body: $(cat "$scratch/health.json")"
[ ! -s "$scratch/python-called" ] || fail "server attempted to launch a Python executable"

kill -INT "$server_pid"
attempt=0
while server_alive && [ "$attempt" -lt 50 ]; do
    attempt=$((attempt + 1))
    sleep 0.1
done
server_alive && fail "server did not stop within five seconds after SIGINT"
set +e
wait "$server_pid"
server_exit=$?
set -e
[ "$server_exit" -eq 0 ] || fail "server exited with status $server_exit after SIGINT"
server_pid=
printf 'rust foundation smoke passed: compiled ssx-server returned native readiness with Python bridge audit clear\n'
