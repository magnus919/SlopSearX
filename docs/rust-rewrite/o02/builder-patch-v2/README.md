# Builder successor after the native verifier and rmcp repair

This additive v2 capsule reconciles PR #1116's Rust 1.99.1 builder update with
published prerequisite `2ec2d4bb50562fb93a22353914d871f6376efcf0` from #1115.
The prerequisite supplies the native verification workflow, authenticated
rmcp 3.9.0 security successor and its compatibility controls. This integration
does not suppress advisories, remove checks or lower browser budgets.

V1 remains unchanged as historical evidence. V2 owns the same three paths and
five ordered edits; its deployment-test parent is the prerequisite's exact
12,945-byte native-workflow oracle, rather than V1's earlier Python-workflow
oracle. Its compiled SHA is
`d9ffb0a941993525d0cfd0a9ee7bda73dc0df064cac3be30e3161d0742095738`.
The existing descriptor-bound physical reader authenticates the runtime capsule
and current source before inverse interpretation. The inverse verifies the
prior size/hash and reconstructs the entire current source forward.

The native image-pin reader consumes the authenticated projected deployment
operand while retaining its independently fixed source SHA, literal pin and
O02 successor metadata checks. Arbitrary fixture roots still use the strict
literal-only grammar; a physical mutation cannot inherit the repository-only
exception. The existing bounded-source test retains physical mutation, growth,
symlink and non-file controls and now includes the runtime capsule in its
owned fixture closure. It checks the projected operand's exact size/hash.

The prerequisite's historical source/lock seals and original F01 inventory
remain unchanged. `t08a-bindings-prior.json` preserves its published binding
bytes; `t08a-binding-audit.json` records the finite current reader/helper/capsule
binding delta. V1's capsule also remains authenticated by the current inventory.

Acceptance requires actual F01/O03/T06/T08a consumers, native image-pin and
physical mutation tests, strict lint, dependency policy, fresh hosted portal
and architecture checks, and independent review of the final exact head.
Rollback reverts the builder integration and capsule together; dependency
security and predecessor history must not be silently rolled back with it.
No deployment or image promotion is authorized by these changes.
