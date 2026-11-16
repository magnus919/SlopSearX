# Owned allocation budget

This dependency-free leaf generalizes A05's `html_budget.rs` accounting. `Budget`
clones share one checked ledger; `Charge` is non-cloneable and supports ownership
transfer by split/merge. Reductions are only valid after covered storage is freed.
Infra retains admission policy and process lifecycle ownership.

`OwnedVec` and `OwnedString` keep backing storage and its private charge together.
Growth reserves the full replacement layout while the old layout remains charged,
uses fallible allocation, checks returned capacity, transfers elements, frees old
storage, then releases the old charge. Nested allocations inside elements require
separate charges. No bare owned storage or mutable retained charge escapes.
Zero-sized elements consume no backing bytes. Layout arithmetic uses checked
`Layout::array`, including alignment and the platform allocation-size bound.

This accounts requested backing storage, not RSS or allocator metadata. The
ledger's Arc/Mutex allocation and stack wrappers are not charged here and must be
included in a consumer's aggregate envelope. `try_reserve_exact` is not assumed
to guarantee exact capacity: an uncovered returned capacity is detected AFTER
allocation, discarded and permanently closes admission. Supported target
allocator qualification and separate process containment remain necessary; this
check alone does not prove there was never a transient over-quota allocation.

A parent must reserve a child allowance in its own ledger and retain that charge
through verified child reap. The child's local ledger represents that reserved
allowance, not additional independent quota. Codec/parser/reply consumers have
not yet migrated, and this crate alone does not establish their memory bound.

Focused tests cover charge transfer/failure preservation, old+new overlap,
injected allocation rollback, alignment/ZST/overflow, unexpected capacity,
poison refusal, element drop ordering and UTF8 ownership. GNU target allocator
qualification is pending; current focused execution is Darwin only.
