# T07 macOS platform-trust: route proposal

Status: **proposal only — nothing in this document has been executed.** Issued
for the owner decision that #1073 asks for. No trust store, authorization
policy, SIP setting or runner image was changed to produce it.

## What fails today

The dedicated macOS witness must reject a unique private CA, establish real
default-platform trust for that CA, prove an unrelated additive bundle does not
replace independently trusted platform roots, reject the wrong hostname, remove
the owned trust and reject again, then restore the keychain search list and
retire every owned process, file and trust record.

At the author head recorded in #1073 the witness times out inside the
establishment step. The measured sequence is:

1. the owned `security` client requests the right with flags `0x13` and logs
   **preauthorization**;
2. `trustd` immediately requests the same right with flags `0x03`;
3. no completed grant or denial is captured before the 15-second operation
   deadline;
4. cleanup and finalizer also fail, so the marker is retained.

The branch helper (`crates/ssx-gateway/tests/platform_support/mac_trust.rs` on
`codex/preview-1056-t07-source-integration-20261027`) issues exactly:

```
security create-keychain -p <owned> <path>
security unlock-keychain -p <owned> <path>
security list-keychains -s <owned> <previous…>
security add-trusted-cert -r trustRoot -k <owned user keychain> <ca.der>
```

The absence of `-d` means the trust is written to the **user** domain, which on
modern macOS requires an interactive session-owner credential. The stored rule
`entitled-session-owner-or-authenticate-session-owner` is what surfaces as the
`0x03` request; preauthorization (`0x13`) does not imply the credential was
obtained. A hosted runner has no interactive session, so the request cannot
complete.

## Candidate routes

Every route below changes something the routine rewrite mandate does **not**
cover. That is the decision being requested.

### A. System trust domain under passwordless `sudo` on the ephemeral runner

Write the same owned certificate into the system keychain instead of a user
keychain, from the same bounded Rust owner, and remove it the same way:

```
sudo security add-trusted-cert -d -r trustRoot -k /Library/Keychains/System.keychain <ca.pem>
sudo security remove-trusted-cert -d <ca.pem>
```

- **Proves:** real default-platform trust for the unique CA (system domain is the
  platform default, not a process-local override), plus the negative controls and
  removal.
- **Authority needed:** permission to mutate the trust store of a disposable
  hosted macOS runner. GitHub-hosted macOS runners run as an admin user with
  passwordless `sudo`; the job is already ephemeral, and the same fixture already
  performs the equivalent privileged mutation on the Linux lane through
  `sudo cp` + `sudo update-ca-certificates`.
- **Risks:** the shared system keychain is wider than an owned user keychain, so
  the finalizer must prove removal and that no unrelated root was disturbed; a
  failed run leaves trust on a VM that is destroyed moments later.
- **Unresolved:** whether `trustd` still requires a session for the *system*
  domain on recent macOS versions. This is the one fact that decides A, and it
  can only be established by an authorized run.

### B. Disposable macOS image with the CA pre-provisioned

Bake the trust anchor into the runner image and rotate the image per release.

- **Proves:** trust exists at boot, removal can be exercised separately.
- **Authority needed:** runner image provisioning.
- **Rejected as stated:** the contract requires a **unique** CA per run; a fixed
  per-image anchor is not unique, and rotating images per job is disproportionate.

### C. Dedicated trust host outside the hosted runner

Run the witness on an owned macOS host with a real logged-in session and explicit
authorization to install a test root.

- **Proves:** the full sequence including the interactive authorization path.
- **Authority needed:** a dedicated host, credentials, and an operator present or
  a pre-authorized session.
- **Cost:** permanent infrastructure for a one-off qualification claim.

### D. Amend the T07 contract for the macOS leg

Keep the automated establish/remove witness on Linux (where `update-ca-certificates`
already proves real system-store trust and removal), and record macOS as an
explicit-CA boundary plus a documented manual trust exercise by a named operator,
with retained screenshots and `security verify-cert` output.

- **Proves:** less. It converts a platform claim into a scoped, honest boundary.
- **Authority needed:** a contract amendment accepted by the owner, not new
  credentials.
- **Cost:** permanent reduction of the macOS acceptance claim.

## Recommendation

**Attempt A first, with D as the fallback.** A keeps the witness real and keeps
every existing negative control; if a single authorized run shows `trustd` still
requires a session for the system domain, that result is decisive and D becomes
the only honest remaining option.

A is deliberately proposed rather than attempted: it mutates a trust store and
touches the platform contract, and #1073 requires a proposal plus independent
review before any new authority is exercised.

## Review and negative controls required before the first real run

The route must be reviewed and exercised with fakes before any authorized
execution, and the run must retain:

1. a fake-command harness proving install, unlock, list, add, remove and
   restore failures each surface as named errors, plus timeout and reap paths;
2. proof the certificate is absent after removal (`security verify-cert`, and a
   dump of the trust settings), not merely that the removal command exited 0;
3. proof an unrelated additive CA bundle does not replace independently trusted
   platform roots;
4. wrong-hostname rejection before and after the establish/remove cycle;
5. restoration of the original keychain search list, with the pre-run list
   recorded;
6. retirement of every owned process and file, and absence of the marker;
7. exact Intel and ARM evidence from the same commit, since the two runners are
   different images.

## Decision requested

Authorize **A** (system-domain trust under `sudo` on the ephemeral macOS runner),
or state that macOS should move to **D** and amend the contract. Anything else —
repeating the user-domain timeout, raising the deadline, switching domains without
review, or activating a personal trust store — stays out of scope by the issue's
own terms.

The Windows counterpart (#1075) has the same shape: its fixture already reaches
the certificate-load boundary and needs the same class of decision about
CurrentUser Root and `certutil` execution on the hosted image. Whichever route is
chosen for macOS should be applied consistently there.
