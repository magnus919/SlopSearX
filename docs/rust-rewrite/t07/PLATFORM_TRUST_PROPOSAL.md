# T07 macOS platform-trust route assessment

This assessment preserves the historical investigation for
[#1073](https://github.com/magnus919/SlopSearX/issues/1073). The maintainer removed
trust-store mutation from the T07 release gate on 2026-11-27.
[ADR 010](../../adr/010-support-apple-silicon-macos.md) defines the required
installed-gateway HTTPS checks. The routes and cleanup criteria below apply
only if the optional investigation resumes with an authorized disposable host.
The Windows investigation in [#1075](https://github.com/magnus919/SlopSearX/issues/1075)
is independent.

## Observed boundary

The retained hosted Intel and ARM evidence records UID 501, an Aqua bootstrap manager
and manager UID 501. The owned `security` process logged preauthorization with flags
`0x13`. A `trustd` request followed with flags `0x03`, and the operation reached its
deadline before a completed grant or denial was captured. Cleanup and the finalizer also
failed, leaving the marker. The stored authorization rule describes a possible decision
path. The effective authorization outcome and whether trust settings were committed or
fully removed remain unproven. [Authenticated evidence and
correction](https://github.com/magnus919/SlopSearX/issues/1073#issuecomment-6711361956)

The fixture used a dynamically owned user keychain and requested a user-domain trust
setting. Apple's API reference describes GUI authentication for per-user trust settings
and retains older wording about root privileges. Apple's Big Sur security release notes
explicitly require administrator authentication for admin-domain changes even when the
caller runs as root. A visible session and passwordless elevation establish different
prerequisites. The fixture still needs an observed, authorized path for both
establishment and removal. [Apple trust-settings
API](https://developer.apple.com/documentation/security/sectrustsettingssettrustsettings%28_%3A_%3A_%3A%29?changes=__7)
· [Apple security release
notes](https://developer.apple.com/documentation/macos-release-notes/macos-big-sur-11_0_1-release-notes?changes=_9)

## Candidate environments

| Environment | Evidence or authority needed |
| --- | --- |
| Hosted macOS with admin-domain trust | Establish a supported administrator authorization path for both adding and removing the unique test CA. Record what authorization the runner can supply. Keep this route on hold while those prerequisites are unproven. |
| Disposable runner with a pre-provisioned image | Show initial rejection and per-run establishment using a unique CA. A fixed anchor baked into the image supplies boot-time trust only. |
| Managed disposable macOS hosts with a per-run certificate profile | Apple documents deployment of root-certificate payloads to managed Macs. A candidate still needs a reviewed qualification environment, a unique test CA and profile per run, authenticated install and removal of that profile, verification that the same installed client returns to rejection, and proof of exact cleanup. The owner must review compatibility with the selected trust contract and authorize device-management changes. [Apple deployment guide](https://support.apple.com/en-mide/guide/deployment/dep91d2eb26/web) · [Apple Developer Technical Support](https://developer.apple.com/forums/thread/820374) |
| Dedicated disposable test hosts with an operator session | Identify a disposable Apple Silicon environment, an operator who can authorize establishment and removal, and a procedure to isolate and restore the host. Review the environment and harness before requesting a test window. |
| Acceptance-scope amendment | Record the exact platform claim being removed or narrowed and obtain maintainer approval in a separate reviewed decision. Explicit CA-bundle success has a different scope from platform-default trust. A failed attempt on any one route leaves the remaining routes open for evaluation. |

## Evidence required for an authorized route

1. Identify the exact candidate commit, installed client artifacts, macOS releases and
   architecture, user identity, selected trust domain, and unique per-run CA
   fingerprint.
2. Run the installed client against the same HTTPS fixture with the default platform
   store. Record initial rejection, acceptance after the owned trust setting is added,
   and rejection for a wrong hostname while the CA is trusted. Record wrong-hostname
   rejection before and after the trust cycle as well. Confirm that adding an unrelated
   explicit CA bundle preserves an independently trusted platform root.
3. Remove the exact test CA trust record and any fixture certificate import. Verify
   removal in the selected domain, record rejection again with the installed client, and
   verify restoration of the original keychain search list.
4. Reap the owned process tree and retire the fixture and marker only after verified
   cleanup. Retain diagnostic evidence for a deadline, an incomplete authorization trace
   or incomplete cleanup.
5. Run the exact sequence on the supported Apple Silicon environment. Before the run,
   independently review source and fake-command evidence for install, removal,
   restoration, timeout, and cleanup paths. Obtain explicit authorization for the
   selected environment and trust operation.

The retained Windows marker precedes completion of the certificate-load
expression. The Windows CurrentUser Root investigation has its own scope.
