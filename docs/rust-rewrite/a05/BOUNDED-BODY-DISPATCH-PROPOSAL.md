# Bounded body dispatcher candidate

Status: implementation candidate, not helper/shared-selector adoption.

Add `decode_http_bounded(body: &[u8], label: &str, budget: &Budget,
checkpoint: &dyn Fn() -> Result<(), BoundedTextError>) ->
Result<BudgetText, BoundedDecodeError>`.

Errors are `Codec(&'static str)` (exact currently representable body diagnostic),
`Output(BoundedTextError)` (including distinct Resource and Deadline), and
`RegistryInvariant`. Source codec errors never absorb resource/deadline failure.
Empty body bypasses label lookup/rejection. Unknown labels select UTF8 like the
existing HTTP facade. This is body decoding: HTTPX/email label selection and its
typed NUL/lookup errors remain the separate selector's responsibility.

Implement identical source normalization using charged OwnedString, including
Unicode16 alphanumeric classification, original-alias then dotted-alias-only
then nondotted-module precedence. Dotted replacement gets an independent charge
while normalized bytes still live. All capacity growth uses reviewed ledger
old+new overlap accounting. Normalization buffers drop before output reservation.
No uncharged String/Vec label clone or call to the legacy allocating decoder.

The output builder reserves checked B codepoints (4B bytes) before any family
work, using the existing all108-codec consumption bound. BOM and rejected-mode
source errors are validated before that reservation. Singlebyte/ASCII/Latin1
emit directly; all Unicode/CJK/ISO/HZ/escape families use their shared fallible
sinks; Punycode uses Prepared and charged arbitrary limbs with checked insertion.
Returned BudgetText owns output charge; temporary Punycode scratch drops first.
Caller separately charges borrowed body/header ownership and aggregate child
allowance, retained via supervisor through verified reap. This leaf does not
invent another independent per-child quota or claim whole-process RSS bounds.

Checkpoint before admission/lookup, per normalized character, per output emission,
and around returned output; Punycode additionally checks input/limb/adapt/insert
loops. Existing family scans and bulk UTF8 validation/copy can still perform work
between emission callbacks. Only the independently supervised process expiry and
kill/reap bound that CPU work; callbacks alone are not a hard latency guarantee.
Run request-owned decoding only inside the existing supervised helper.

Qualification: complete original117-codec policy/aliases/applicability outcomes,
complete family captures under charged output, Unicode16/dotted alias controls,
empty/unknown/rejected/BOM precedence, one-byte-short output/label/scratch quotas,
independent charge lifetime and typed deadline refusal. Strict parameter/IDNA
bounded decoding and full helper/protocol/aggregate qualification remain separate
open work; this method is not an implicit fallback to those unbounded paths.
