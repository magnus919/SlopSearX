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
    fingerprint="$(shasum -a 1 "$case_root/ca.der" | cut -d ' ' -f1)"
    [[ "$fingerprint" =~ ^[0-9a-f]{40}$ ]]
    keychain="$case_root/platform-fixture.keychain-db"
    if [[ "$mode" == install ]]; then
      [[ ! -e "$keychain" ]]
      security list-keychains -d user > "$case_root/original-keychains"
      printf '%s\n' "$fingerprint" > "$case_root/trust-active"
      security create-keychain -p ephemeral-platform-fixture "$keychain"
      security unlock-keychain -p ephemeral-platform-fixture "$keychain"
      # Preserve the existing search list; append only this owned keychain.
      keychains=()
      while IFS= read -r line; do
        line="${line#*\"}"; line="${line%\"*}"
        [[ -n "$line" ]] && keychains+=("$line")
      done < "$case_root/original-keychains"
      security list-keychains -d user -s "${keychains[@]}" "$keychain"
      security add-trusted-cert -r trustRoot -k "$keychain" "$case_root/ca.pem"
    elif [[ "$mode" == remove ]]; then
      if [[ -f "$case_root/trust-active" ]]; then
        [[ "$(cat "$case_root/trust-active")" == "$fingerprint" ]]
        # Remove trust for the exact certificate, then the exclusively owned store.
        # Attempt all cleanup stages even when an earlier one fails.
        cleanup_status=0
        security remove-trusted-cert "$case_root/ca.pem" || cleanup_status=1
        keychains=()
        while IFS= read -r line; do
          line="${line#*\"}"; line="${line%\"*}"
          [[ -n "$line" ]] && keychains+=("$line")
        done < "$case_root/original-keychains"
        security list-keychains -d user -s "${keychains[@]}" || cleanup_status=1
        if [[ -f "$keychain" ]]; then security delete-keychain "$keychain" || cleanup_status=1; fi
        [[ "$cleanup_status" == 0 ]]
        rm -- "$case_root/trust-active"
      fi
    else exit 2; fi
    ;;
  *) exit 2 ;;
esac
