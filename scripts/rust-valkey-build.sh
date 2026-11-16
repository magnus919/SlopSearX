#!/bin/sh
# Test dependency only. Builds the same official pinned source on both CI CPUs.
set -eu
valkey_version=8.1.10
valkey_sha256=c74e50cd83f6d398a3dc570e04ac2fe538249585d021f76ee2449bbf9ebd04ed
valkey_dest=${1:?usage: rust-valkey-build.sh ABSOLUTE_DESTINATION}
case "$valkey_dest" in /*) ;; *) echo 'destination must be absolute' >&2; exit 1 ;; esac
valkey_build=$(mktemp -d)
trap 'rm -rf "$valkey_build"' EXIT HUP INT TERM
curl --fail --location --connect-timeout 15 --max-time 120 \
  "https://github.com/valkey-io/valkey/archive/refs/tags/$valkey_version.tar.gz" \
  --output "$valkey_build/source.tar.gz"
if command -v sha256sum >/dev/null 2>&1; then
  valkey_actual=$(sha256sum "$valkey_build/source.tar.gz")
else
  valkey_actual=$(shasum -a 256 "$valkey_build/source.tar.gz")
fi
valkey_actual=${valkey_actual%% *}
[ "$valkey_actual" = "$valkey_sha256" ] || { echo 'Valkey source checksum mismatch' >&2; exit 1; }
tar -xzf "$valkey_build/source.tar.gz" -C "$valkey_build"
make -C "$valkey_build/valkey-$valkey_version" -j2 MALLOC=libc BUILD_TLS=no valkey-server
mkdir -p "$valkey_dest"
cp "$valkey_build/valkey-$valkey_version/src/valkey-server" "$valkey_dest/valkey-server"
"$valkey_dest/valkey-server" --version | grep -F "v=$valkey_version "
printf '%s\n' "official-source-sha256=$valkey_sha256"
