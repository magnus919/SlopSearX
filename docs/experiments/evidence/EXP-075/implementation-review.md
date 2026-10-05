# EXP-075 implementation review

Independent GPT-6 Luna review is in progress. This record does not qualify the
harness or the ranking candidate. No live EXP-075 request has been admitted.

Material findings from the first review pass:

- The programmatic live runner accepted caller-supplied qualification/proof
  bytes without requiring the gate and could substitute the offline credential
  marker. Require a bound, one-use admission capability and a real nonempty key.
- Environment-selected lease directories could evade the durable exclusive
  study identity. Use a fixed live identity store; admission tests isolate home
  and forbid network/credential access.
- Aggregate usage checks did not reserve the permitted per-call input/output
  allowance before dispatch. Refuse dispatch when remaining budget cannot
  cover those bounds; retain unknown-usage terminal stopping.
- Source closure omitted the actual service reranking seam. Add service.py to
  the exact qualification closure without claiming the harness ships that path.

Root review additionally corrected baseline parse timing, durable failure
endpoints, inconsistent fallback status, and successful response receipt writes.
The earlier synthetic builder failure is retained separately.

All material findings require fixes, affected offline tests and a bounded
follow-up before qualification. The full production acceptance checklist and
untouched task/pool confirmation remain open.
