# Starting confirmation after development passes

Confirmation requires a separate, verified development pass. A prepared config or a successful setup check is not enough.

The development coordinator first finishes and seals its inventory and closeout files. In a separate process, an operator then runs the independent decision command with the inventory and closeout hashes returned by that completed run:

```sh
.venv/bin/python -B scripts/coverage_operator_launcher.py issue-development-pass \
  --config /path/to/development-config.json \
  --config-sha256 "$DEVELOPMENT_CONFIG_SHA256" \
  --inventory-sha256 "$INVENTORY_SHA256" \
  --closeout-sha256 "$CLOSEOUT_SHA256" \
  --output /path/to/private/development-pass.json
```

The output directory must already be private (mode `0700`). The command rechecks the closed evidence, all registered quality gates, both pool-size bands when declared, and actual elapsed time. It refuses failed, incomplete, late, or mismatched evidence. The receipt is written once with mode `0600`; an existing output cannot be replaced or resumed. This scientific pass permits considering confirmation; it does not authorize a product release or deployment.

Before starting confirmation, its independently pinned config must include:

- `development_stage_uuid`: the original development stage UUID.
- `development_receipt_sha256`: the SHA-256 of the exact pass-receipt bytes.
- `private_paths.development_pass_receipt`: the receipt's absolute path, alongside the existing private paths.

The config hash must be updated and checked through the normal registration process. Runtime validation checks this receipt before any confirmation search, capture, or model dispatch. The same bytes and identity travel through the later receipt handoffs, where the issuer checks them again. Development configs reject these confirmation-only fields.

The existing source rule remains strict: development and confirmation must match the registered protocol, source revision, and source inventory. A development receipt from an earlier source cannot be relabelled for a newer one. Any prospective change to that rule needs its own explicit, reviewed study contract before confirmation begins. Earlier registrations and running studies retain their original source identity.

A setup-only preflight can inspect an unstarted confirmation config before its development receipt exists. That establishes configuration readiness only; runtime still refuses to start without a valid pass.
