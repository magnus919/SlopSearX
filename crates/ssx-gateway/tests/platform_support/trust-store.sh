#!/usr/bin/env bash
# Only a unique test CA inside a dedicated ephemeral Actions job may be trusted.
set -euo pipefail
[[ "${GITHUB_ACTIONS:-}" == true && "${SLOPSEARX_PLATFORM_TRUST_FIXTURE:-}" == 1 ]]
mode="$1"
case_root="$2"
[[ -d "$case_root" && -f "$case_root/ca.der" && -f "$case_root/ca.pem" ]]
case "$(uname -s)" in
  MINGW*|MSYS*|CYGWIN*)
    powershell.exe -NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "$(dirname "$0")/trust-store.ps1" "$mode" "$case_root"
    ;;
  Linux)
    fingerprint="$(sha256sum "$case_root/ca.der" | cut -d ' ' -f1)"
    [[ "$fingerprint" =~ ^[0-9a-f]{64}$ ]]
    certificate="/usr/local/share/ca-certificates/ssx-platform-${fingerprint}.crt"
    if [[ "$mode" == install ]]; then
      [[ ! -e "$certificate" ]]
      # Record the exact owned installation before the privileged mutation.
      printf '%s\n' "$certificate" > "$case_root/trust-active"
      sudo cp "$case_root/ca.pem" "$certificate"
      sudo update-ca-certificates
    elif [[ "$mode" == remove ]]; then
      if [[ -f "$case_root/trust-active" ]]; then
        [[ "$(cat "$case_root/trust-active")" == "$certificate" ]]
        sudo rm -f -- "$certificate"
        sudo update-ca-certificates
        rm -- "$case_root/trust-active"
      fi
    else exit 2; fi
    ;;
  Darwin)
    # Every Darwin security child is owned by the bounded Rust helper, including
    # cleanup retries. Never bypass it from an EXIT trap or this shell adapter.
    echo 'Darwin trust fixtures require the bounded t07_platform_tls Rust owner' >&2
    exit 2
    ;;
  *) exit 2 ;;
esac
