# Offline intent-ranking response receipts

`scripts/intent_ranking_receipts.py` provides a small, offline archive and replay seam for raw selector responses. It accepts only the exact response body and a canonical receipt; it never accepts headers or request payloads. Raw bodies remain private evidence and must not be committed or treated as sanitized. The request body is represented by its SHA-256 digest.

An archive is bounded to 2,000,000 response bytes. A slot is keyed by a canonical stage UUID and a safe operation ID. New stage and operation directories use mode `0700`; response and receipt files use `0600`. Writes are exclusive and fsynced. Existing slots are never overwritten or completed by a later write, including empty slots left by interruption. Partial artifacts remain on disk and fail replay checks. The caller-owned archive root must already exist and must not be a symlink. The implementation requires POSIX permissions and directory fsync; unavailable durability guarantees fail closed. It does not promise Windows portability.

Transport failures may record an unknown HTTP status as `null`; they remain ineligible for replay.

Replay requires the externally sealed receipt SHA-256 and exact expected bindings: stage UUID, operation ID, request-body SHA-256, and 40-character source revision. Replay validates the receipt schema and types, completion/HTTP status, file modes and inventory, response size, and body digest, then returns the original bytes unchanged. `verify_inventory` additionally requires an exact expected slot inventory and rejects missing or extra slots.

The archive does not decode or interpret response bodies. Callers must pass replayed bytes to the unchanged original parser; malformed or non-UTF-8 bodies are preserved so that parser can reject them. This module grants no execution authority and makes no scientific or quality claim.

Run its tests with:

```bash
python -m unittest tests.test_intent_ranking_receipts
```
