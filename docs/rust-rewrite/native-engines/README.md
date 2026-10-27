# Additive native engines

`catalog.json` declares Rust-only additions beyond the frozen 51-engine F06
migration inventory. Runtime `Registry::builtins()` composes both catalogs and
rejects duplicate names or invalid descriptors. `Registry::migration_baseline()`
keeps the historical parity set available to the F06 gate. Original F06 fragments,
source pins, routing expectations and captures remain unchanged.

F06 validates both compiled sets separately; registry tests check original
per-engine equality, native completeness, routing and duplicate rejection.
DeepSeek's factory/configuration, transport, service/cache and HTTP/HTML tests
prove its executable availability. Static metadata never grants dispatch authority;
configuration, factory binding, credentials, policy and health remain authoritative.

Native additions need their own descriptor and tests, documentation, shared-policy
and portal review. Do not add a Python adapter just to extend a native catalog.
See [DeepSeek operator and integration guide](../../DEEPSEEK.md).

The config/registry source amendment reconstructs the reviewed native additions
against all thirteen original A10–A13 caller tuples, including hash-only rows.
The factory amendment separately reconstructs its exact module insertion. Both
preserve frozen manifests and authenticate actual current files before exposing
historical proof operands. `native_catalog_source` exercises the original A13
entrypoint and wrong-caller, length, mode, edit, and rehashed-change controls.
