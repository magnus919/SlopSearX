//! Closed portable-unit/CI child of the already accepted main and workflow bytes.
use super::super::{digest, edits, material, need, permissions, read, text};
use serde_json::{Value, json};
use std::path::Path;
const CAPSULE: &str =
    "docs/rust-rewrite/t07/source-amendments/gateway-unit-route-successor-v1.json";
const SHA: &str = "4fabaf5668fbe765a315e1399d54138db4d51e0342fc16331e0e791cdc6cacf2";
const BYTES: u64 = 17885;
const PATHS: [&str; 3] = [
    "crates/xtask/src/main.rs",
    ".github/workflows/rust.yml",
    ".pre-commit-config.yaml",
];
fn document(raw: &[u8]) -> Result<Value, String> {
    need(digest(raw) == SHA, "T07 unit-route capsule digest")?;
    let doc: Value = serde_json::from_slice(raw).map_err(|e| e.to_string())?;
    need(
        doc["kind"] == "T07_PORTABLE_UNIT_ROUTE_SUCCESSOR_V1"
            && doc["source_parent"] == "0f77197409ced9454e9930521f147f76fdd3c557"
            && doc["paths"] == json!(PATHS),
        "T07 unit-route exact closed paths",
    )?;
    let rows = doc["transitions"]
        .as_array()
        .ok_or("T07 unit-route transitions")?;
    need(
        rows.len() == PATHS.len(),
        "T07 unit-route closed transition count",
    )?;
    for (row, path) in rows.iter().zip(PATHS) {
        need(
            row["prior"]["path"] == path
                && row["current"]["path"] == path
                && row["archive"]["source_path"] == path
                && row["archive"]["path"]
                    == format!(
                        "docs/rust-rewrite/t07/source-amendments/gateway-unit-route-predecessor-v1/{path}"
                    ),
            "T07 unit-route exact material paths",
        )?;
    }
    Ok(doc)
}
fn load(root: &Path) -> Result<Value, String> {
    document(&read(
        root,
        &json!({"path":CAPSULE,"sha256":SHA,"bytes":BYTES,"mode":"0644"}),
    )?)
}
fn row<'a>(doc: &'a Value, expected: &Value) -> Result<&'a Value, String> {
    let path = text(expected, "path")?;
    need(PATHS.contains(&path), "T07 unit-route unlisted caller")?;
    let matches: Vec<_> = doc["transitions"]
        .as_array()
        .ok_or("T07 unit-route rows")?
        .iter()
        .filter(|r| r["prior"]["path"] == path)
        .collect();
    need(
        matches.len() == 1 && matches[0]["prior"] == *expected,
        "T07 unit-route exact predecessor caller",
    )?;
    Ok(matches[0])
}
fn validate(row: &Value, before: &[u8], actual: &[u8], mode: &str) -> Result<(), String> {
    material(&row["prior"], before, "0644")?;
    material(&row["current"], actual, mode)?;
    need(
        edits(before, &row["edits"])? == actual,
        "T07 unit-route exact ordered reconstruction",
    )
}
pub(super) fn archived_prior(root: &Path, expected: &Value) -> Result<Vec<u8>, String> {
    let doc = load(root)?;
    read(root, &row(&doc, expected)?["archive"])
}
const MAIN_PARENT_INVERSE: &str = r###"[{"at":438,"remove_utf8":"mod t02;\n","insert_utf8":""},{"at":18084,"remove_utf8":"        \"T02\" => t02::check(root, records),\n","insert_utf8":""},{"at":37684,"remove_utf8":"            execute(&root, \"T03\", &mut vec![])\n","insert_utf8":"            execute(&root, \"T02\", &mut vec![])\n"}]"###;
fn main_frame(
    expected: &Value,
    parent: &[u8],
    mode: &str,
    inverse: &Value,
) -> Result<Vec<u8>, String> {
    need(
        *expected
            == json!({"path":"crates/xtask/src/main.rs","bytes":40204,"sha256":"272553def5ba145a76aa2908eb68f3a0d8f5464363927201f461bd647a7c53ce","git_blob":"4183621145ef958633910778157f6f802d17f584","mode":"0644"})
            && mode == "0644"
            && parent.len() == 40257
            && digest(parent) == "9a26d3aa5874ba896fff576d97a02d344fdc39922508e4e94f8a1e8dc57eef60",
        "T07 unit-route exact parent/caller frame differs",
    )?;
    need(
        digest(&serde_json::to_vec(inverse).map_err(|e| e.to_string())?)
            == "77ba4e7a9f852f145b9b21ed805c895ceda1168e98d12d2d220811ed0927750a",
        "T07 unit-route closed three-span inverse differs",
    )?;
    let prior = edits(parent, inverse)?;
    material(expected, &prior, "0644")?;
    Ok(prior)
}
fn current_main(root: &Path, expected: &Value) -> Result<Vec<u8>, String> {
    let parent = super::super::main_isolation_current::parent(
        root,
        "crates/xtask/src/main.rs",
        None,
        super::super::main_isolation_current::Parent::T07,
    )?;
    let inverse: Value = serde_json::from_str(MAIN_PARENT_INVERSE).map_err(|e| e.to_string())?;
    let prior = main_frame(
        expected,
        &parent,
        &permissions(&root.join("crates/xtask/src/main.rs"))?,
        &inverse,
    )?;
    let git = std::process::Command::new("git")
        .args([
            "cat-file",
            "blob",
            "4183621145ef958633910778157f6f802d17f584",
        ])
        .current_dir(root)
        .output()
        .map_err(|e| e.to_string())?;
    need(
        git.status.success() && git.stdout == prior,
        "T07 unit-route exact Git frame differs",
    )?;
    Ok(prior)
}
// Only the two remaining closed unit-route configuration rows use these
// accepted current-to-capsule inverses; neither target config is modified.
fn config_spec(
    path: &str,
) -> Result<(Value, usize, &'static str, &'static str, &'static str), String> {
    match path {
        ".github/workflows/rust.yml" => Ok((
            json!({"path":".github/workflows/rust.yml","bytes":25236,"sha256":"3a5b3c1d8c6988a50bc00b9a7d51c1789c81e38580d1448bae6e61c6b1a02560","git_blob":"d1d5a9d73c1c55b15f2cadbd96cf5dd52c828420","mode":"0644"}),
            53956,
            "e511701afc001bbd3eb160f16cc029b7ed2a0e1f73f6d5953ad056cb21c10937",
            "3862774edb386e8c8f0b93a23e38110eae15e61aa6df20ed2c164579d2343e33",
            r###"[{"at":131,"remove_utf8":"  foundation-prerequisites:\n    name: Foundation prerequisites (${{ matrix.runner }})\n","insert_utf8":"  foundation:\n    name: Foundation (${{ matrix.runner }})\n"},{"at":251,"remove_utf8":"    timeout-minutes: 120\n","insert_utf8":"    timeout-minutes: 90\n"},{"at":4363,"remove_utf8":"      - name: Fresh source-bound workspace compiler provenance\n        shell: bash\n        run: |\n          set -euo pipefail\n          umask 077\n          evidence=\"target/rust-evidence/partitions/prerequisites\"\n          mkdir -p \"$evidence\"\n          git ls-files -z | LC_ALL=C sort -z | xargs -0 sha256sum --zero > \"$evidence/source-before.sha256z\"\n          git ls-files --stage -z > \"$evidence/stage-before.z\"\n          [ \"$(git rev-parse HEAD)\" = \"$GITHUB_SHA\" ]\n          [ -z \"$(git status --porcelain)\" ]\n          rustc --version --verbose > \"$evidence/rustc.txt\"\n          cargo --version --verbose > \"$evidence/cargo.txt\"\n          sha256sum Cargo.toml Cargo.lock rust-toolchain.toml > \"$evidence/build-inputs.sha256\"\n          cargo build --locked --workspace --all-targets --message-format=json-render-diagnostics > \"$evidence/workspace.jsonl\"\n          PARTITION=prerequisites TARGET_TRIPLE=\"${{ matrix.target }}\" node <<'NODE'\n          const fs = require('node:fs'), path = require('node:path'), crypto = require('node:crypto');\n          const root = fs.realpathSync(process.cwd()), dir = `target/rust-evidence/partitions/${process.env.PARTITION}`;\n          const rows = fs.readFileSync(`${dir}/workspace.jsonl`, 'utf8').trim().split('\\n').map(JSON.parse);\n          const need = (v, s) => { if (!v) throw Error(s); };\n          need(rows.at(-1)?.reason === 'build-finished' && rows.at(-1).success === true, 'compiler completion');\n          const selected = rows.filter(r => r.reason === 'compiler-artifact' && r.executable);\n          need(selected.length > 0, 'actual executable contexts');\n          const images = selected.map(artifact => {\n            const exe = path.resolve(artifact.executable), rel = path.relative(path.join(root, 'target'), exe);\n            need(rel && !rel.startsWith('..') && !path.isAbsolute(rel), 'compiler target confinement');\n            need(artifact.filenames.includes(artifact.executable), 'declared executable filename');\n            const info = fs.lstatSync(exe); need(info.isFile() && !info.isSymbolicLink(), 'regular compiler executable');\n            return { artifact, sha256: crypto.createHash('sha256').update(fs.readFileSync(exe)).digest('hex'), bytes: info.size, mode: info.mode & 0o777 };\n          });\n          fs.writeFileSync(`${dir}/compiler-images.json`, JSON.stringify(images, null, 2) + '\\n');\n          fs.writeFileSync(`${dir}/identity.json`, JSON.stringify({ partition: process.env.PARTITION, source_revision: process.env.GITHUB_SHA, target: process.env.TARGET_TRIPLE, runner_arch: process.env.RUNNER_ARCH, run_id: process.env.GITHUB_RUN_ID, run_attempt: process.env.GITHUB_RUN_ATTEMPT, evidence_scope: 'actual pre-gate compiler contexts and executable hashes; no immutable byte-archive or OS launch-image claim' }, null, 2) + '\\n');\n          NODE\n\n","insert_utf8":""},{"at":7472,"remove_utf8":"      - name: Build actual-process HTTP fixture server\n        run: cargo build --locked -p ssx-http --example t02_fixture\n","insert_utf8":""},{"at":7643,"remove_utf8":"        env:\n          SLOPSEARX_TEST_TEMP_ROOT: ${{ runner.temp }}/t02-sockets-workspace\n          TMPDIR: ${{ runner.temp }}/t02-sockets-workspace\n          TMP: ${{ runner.temp }}/t02-sockets-workspace\n          TEMP: ${{ runner.temp }}/t02-sockets-workspace\n        run: |\n          mkdir \"$SLOPSEARX_TEST_TEMP_ROOT\"\n          trap 'rmdir \"$SLOPSEARX_TEST_TEMP_ROOT\" 2>/dev/null || true' EXIT\n          cargo test --locked --workspace\n","insert_utf8":"        run: cargo test --locked --workspace\n"},{"at":19132,"remove_utf8":"      - name: Verify prerequisites source and index unchanged\n        if: always()\n        shell: bash\n        run: |\n          set -euo pipefail\n          evidence=\"target/rust-evidence/partitions/prerequisites\"\n          git ls-files -z | LC_ALL=C sort -z | xargs -0 sha256sum --zero > \"$evidence/source-after.sha256z\"\n          git ls-files --stage -z > \"$evidence/stage-after.z\"\n          cmp \"$evidence/source-before.sha256z\" \"$evidence/source-after.sha256z\"\n          cmp \"$evidence/stage-before.z\" \"$evidence/stage-after.z\"\n          [ \"$(git rev-parse HEAD)\" = \"$GITHUB_SHA\" ]\n          [ -z \"$(git status --porcelain)\" ]\n\n      - name: Retain prerequisites foundation partition evidence\n        if: always()\n        uses: actions/upload-artifact@v7\n        with:\n          name: rust-foundation-prerequisites-${{ matrix.target }}-${{ github.sha }}\n          path: |\n            target/rust-evidence/\n            target/rust-feature-graph.txt\n          if-no-files-found: warn\n\n      - name: Retire gateway package fixtures\n        if: always()\n        run: rm -rf -- \"$SLOPSEARX_INSTALL_TEST_ROOT\"\n\n  foundation-runtime:\n    name: Foundation runtime (${{ matrix.runner }})\n    needs: foundation-prerequisites\n    runs-on: ${{ matrix.runner }}\n    timeout-minutes: 120\n    strategy:\n      fail-fast: false\n      matrix:\n        include:\n          - runner: ubuntu-24.04\n            target: x86_64-unknown-linux-gnu\n          - runner: ubuntu-24.04-arm\n            target: aarch64-unknown-linux-gnu\n    env:\n      CARGO_TERM_COLOR: always\n      RUST_BACKTRACE: 1\n      SLOPSEARX_INSTALL_TEST_ROOT: ${{ runner.temp }}/ssx-gateway-install\n      SLOPSEARX_T07_UNIT_TEST_ROOT: ${{ runner.temp }}/ssx-t07-unit-tests\n    steps:\n      - uses: actions/checkout@v7\n        with:\n          fetch-depth: 0\n\n      - name: Prepare private gateway package fixtures\n        run: mkdir -m 700 \"$SLOPSEARX_INSTALL_TEST_ROOT\"\n      - name: Prepare dedicated portable unit fixture scratch\n        run: mkdir -p \"$SLOPSEARX_T07_UNIT_TEST_ROOT\"\n\n      - name: Confirm pinned toolchain\n        run: rustc --version --verbose\n\n      - name: Install mandatory offline replay isolation\n        run: |\n          sudo apt-get update\n          sudo apt-get install --yes bubblewrap apparmor\n          /usr/bin/bwrap --version\n          # Same namespace/capability flags as the native runner; no candidate runs here.\n          replay_setup_probe() {\n            /usr/bin/bwrap --unshare-user --unshare-net --unshare-pid --disable-userns \\\n              --cap-drop ALL --new-session --die-with-parent --ro-bind / / \\\n              --proc /proc --dev /dev -- /bin/sh -c \\\n              'cat /proc/self/attr/current; grep \"^CapEff:\" /proc/self/status; cat /proc/net/dev'\n          }\n          sudo cat /sys/kernel/security/apparmor/profiles | grep -E '^(bwrap|unpriv_bwrap)' || true\n          # Refuse an unexpected executable attachment instead of layering a second profile.\n          for policy in /etc/apparmor.d/*; do\n            if [ -f \"$policy\" ] && grep -Eq '^[[:space:]]*(profile[[:space:]]+[^[:space:]]+[[:space:]]+)?/usr/bin/bwrap([[:space:]]|$)' \"$policy\"; then\n              echo \"Existing bubblewrap policy: $policy\"\n              cat \"$policy\"\n              if ! grep -Eq '^[[:space:]]*profile[[:space:]]+bwrap[[:space:]]+/usr/bin/bwrap([[:space:]]|$)' \"$policy\"; then\n                echo \"Unexpected bwrap attachment identity; refusing policy replacement\" >&2\n                exit 1\n              fi\n            fi\n          done\n          if ! replay_setup_probe; then\n            sudo journalctl -k --no-pager -n 100 | grep -E 'apparmor|userns|bwrap' || true\n          fi\n          # Purpose-built bwrap setup profile plus capability-denied child stack.\n          # See docs/rust-rewrite/replay/README.md for upstream policy provenance.\n          sudo apparmor_parser --replace <<'PROFILE'\n          abi <abi/4.0>,\n          include <tunables/global>\n          profile bwrap /usr/bin/bwrap flags=(attach_disconnected,mediate_deleted) {\n            capability,\n            file rwlkm /{**,},\n            network,\n            unix,\n            ptrace,\n            signal,\n            userns,\n            mount,\n            umount,\n            pivot_root,\n            dbus,\n            pix /** -> &bwrap//&ssx_replay_unpriv,\n          }\n          profile ssx_replay_unpriv flags=(attach_disconnected,mediate_deleted) {\n            file rwlkm /{**,},\n            network,\n            unix,\n            ptrace,\n            signal,\n            userns,\n            mount,\n            umount,\n            pivot_root,\n            dbus,\n            pix /** -> &ssx_replay_unpriv,\n            audit deny capability,\n          }\n          PROFILE\n          replay_setup_probe\n\n      - name: Build pinned disposable native Valkey\n        timeout-minutes: 15\n        run: |\n          sh scripts/rust-valkey-build.sh \"$RUNNER_TEMP/ssx-valkey/bin\"\n          echo \"$RUNNER_TEMP/ssx-valkey/bin\" >> \"$GITHUB_PATH\"\n          valkey_server=\"$RUNNER_TEMP/ssx-valkey/bin/valkey-server\"\n          {\n            printf 'SLOPSEARX_TEST_VALKEY_SERVER=%s\\n' \"$valkey_server\"\n            printf 'SLOPSEARX_TEST_VALKEY_SERVER_SHA256=%s\\n' \"$(sha256sum \"$valkey_server\" | cut -d ' ' -f 1)\"\n          } >> \"$GITHUB_ENV\"\n\n      - name: Build actual-process HTTP fixture server\n        run: cargo build --locked -p ssx-http --example t02_fixture\n\n      - name: Locked workspace build\n        run: cargo build --locked --workspace --target \"${{ matrix.target }}\"\n\n      - name: Verify outbound TLS feature graph\n        run: |\n          cargo tree --locked --workspace -e features > target/rust-feature-graph.txt\n          grep -q 'rustls-platform-verifier' target/rust-feature-graph.txt\n          if grep -Eq '(^|[[:space:]])(native-tls|hyper-tls|openssl-sys)([[:space:]@v]|$)' target/rust-feature-graph.txt; then\n            echo \"unexpected native-TLS/OpenSSL backend in the locked feature graph\" >&2\n            exit 1\n          fi\n\n      - name: Fresh source-bound workspace compiler provenance\n        shell: bash\n        run: |\n          set -euo pipefail\n          umask 077\n          evidence=\"target/rust-evidence/partitions/runtime\"\n          mkdir -p \"$evidence\"\n          git ls-files -z | LC_ALL=C sort -z | xargs -0 sha256sum --zero > \"$evidence/source-before.sha256z\"\n          git ls-files --stage -z > \"$evidence/stage-before.z\"\n          [ \"$(git rev-parse HEAD)\" = \"$GITHUB_SHA\" ]\n          [ -z \"$(git status --porcelain)\" ]\n          rustc --version --verbose > \"$evidence/rustc.txt\"\n          cargo --version --verbose > \"$evidence/cargo.txt\"\n          sha256sum Cargo.toml Cargo.lock rust-toolchain.toml > \"$evidence/build-inputs.sha256\"\n          cargo build --locked --workspace --all-targets --message-format=json-render-diagnostics > \"$evidence/workspace.jsonl\"\n          PARTITION=runtime TARGET_TRIPLE=\"${{ matrix.target }}\" node <<'NODE'\n          const fs = require('node:fs'), path = require('node:path'), crypto = require('node:crypto');\n          const root = fs.realpathSync(process.cwd()), dir = `target/rust-evidence/partitions/${process.env.PARTITION}`;\n          const rows = fs.readFileSync(`${dir}/workspace.jsonl`, 'utf8').trim().split('\\n').map(JSON.parse);\n          const need = (v, s) => { if (!v) throw Error(s); };\n          need(rows.at(-1)?.reason === 'build-finished' && rows.at(-1).success === true, 'compiler completion');\n          const selected = rows.filter(r => r.reason === 'compiler-artifact' && r.executable);\n          need(selected.length > 0, 'actual executable contexts');\n          const images = selected.map(artifact => {\n            const exe = path.resolve(artifact.executable), rel = path.relative(path.join(root, 'target'), exe);\n            need(rel && !rel.startsWith('..') && !path.isAbsolute(rel), 'compiler target confinement');\n            need(artifact.filenames.includes(artifact.executable), 'declared executable filename');\n            const info = fs.lstatSync(exe); need(info.isFile() && !info.isSymbolicLink(), 'regular compiler executable');\n            return { artifact, sha256: crypto.createHash('sha256').update(fs.readFileSync(exe)).digest('hex'), bytes: info.size, mode: info.mode & 0o777 };\n          });\n          fs.writeFileSync(`${dir}/compiler-images.json`, JSON.stringify(images, null, 2) + '\\n');\n          fs.writeFileSync(`${dir}/identity.json`, JSON.stringify({ partition: process.env.PARTITION, source_revision: process.env.GITHUB_SHA, target: process.env.TARGET_TRIPLE, runner_arch: process.env.RUNNER_ARCH, run_id: process.env.GITHUB_RUN_ID, run_attempt: process.env.GITHUB_RUN_ATTEMPT, evidence_scope: 'actual pre-gate compiler contexts and executable hashes; no immutable byte-archive or OS launch-image claim' }, null, 2) + '\\n');\n          NODE\n\n","insert_utf8":""},{"at":27987,"remove_utf8":"        env:\n          SLOPSEARX_TEST_TEMP_ROOT: ${{ runner.temp }}/t02-sockets-f03\n          TMPDIR: ${{ runner.temp }}/t02-sockets-f03\n          TMP: ${{ runner.temp }}/t02-sockets-f03\n          TEMP: ${{ runner.temp }}/t02-sockets-f03\n        run: |\n          mkdir \"$SLOPSEARX_TEST_TEMP_ROOT\"\n          trap 'rmdir \"$SLOPSEARX_TEST_TEMP_ROOT\" 2>/dev/null || true' EXIT\n          cargo xtask check F03 --output target/rust-evidence/F03.json\n","insert_utf8":"        run: cargo xtask check F03 --output target/rust-evidence/F03.json\n"},{"at":29308,"remove_utf8":"      - name: Prepare complete genuine SDK unit root\n        run: cargo xtask prepare-t07-sdk-unit --profile-dir \"$RUNNER_TEMP/ssx-t07-sdk-unit\"\n      - name: Compile exact locked SDK unit profile before offline T07 controls\n        run: cargo xtask bootstrap-t07-sdk-unit --profile-dir \"$RUNNER_TEMP/ssx-t07-sdk-unit\"\n","insert_utf8":""},{"at":29851,"remove_utf8":"          SLOPSEARX_T07_SDK_UNIT_PROFILE_ROOT: ${{ runner.temp }}/ssx-t07-sdk-unit\n","insert_utf8":""},{"at":30126,"remove_utf8":"        env:\n          SLOPSEARX_TEST_TEMP_ROOT: ${{ runner.temp }}/t02-sockets-f05\n          TMPDIR: ${{ runner.temp }}/t02-sockets-f05\n          TMP: ${{ runner.temp }}/t02-sockets-f05\n          TEMP: ${{ runner.temp }}/t02-sockets-f05\n        run: |\n          mkdir \"$SLOPSEARX_TEST_TEMP_ROOT\"\n          trap 'rmdir \"$SLOPSEARX_TEST_TEMP_ROOT\" 2>/dev/null || true' EXIT\n          cargo xtask check F05 --output target/rust-evidence/F05.json\n","insert_utf8":"        run: cargo xtask check F05 --output target/rust-evidence/F05.json\n"},{"at":31573,"remove_utf8":"\n      - name: Native HTTP original corpus and real-process evidence\n        env:\n          SLOPSEARX_TEST_TEMP_ROOT: ${{ runner.temp }}/t02-sockets\n          TMPDIR: ${{ runner.temp }}/t02-sockets\n          TMP: ${{ runner.temp }}/t02-sockets\n          TEMP: ${{ runner.temp }}/t02-sockets\n        run: |\n          mkdir \"$SLOPSEARX_TEST_TEMP_ROOT\"\n          trap 'rmdir \"$SLOPSEARX_TEST_TEMP_ROOT\" 2>/dev/null || true' EXIT\n          cargo xtask check T02 --output target/rust-evidence/T02.json\n","insert_utf8":""},{"at":34414,"remove_utf8":"      - name: Verify runtime source and index unchanged\n        if: always()\n        shell: bash\n        run: |\n          set -euo pipefail\n          evidence=\"target/rust-evidence/partitions/runtime\"\n          git ls-files -z | LC_ALL=C sort -z | xargs -0 sha256sum --zero > \"$evidence/source-after.sha256z\"\n          git ls-files --stage -z > \"$evidence/stage-after.z\"\n          cmp \"$evidence/source-before.sha256z\" \"$evidence/source-after.sha256z\"\n          cmp \"$evidence/stage-before.z\" \"$evidence/stage-after.z\"\n          [ \"$(git rev-parse HEAD)\" = \"$GITHUB_SHA\" ]\n          [ -z \"$(git status --porcelain)\" ]\n\n      - name: Retain runtime foundation partition evidence\n        if: always()\n        uses: actions/upload-artifact@v7\n        with:\n          name: rust-foundation-runtime-${{ matrix.target }}-${{ github.sha }}\n          path: |\n            target/rust-evidence/\n            target/rust-feature-graph.txt\n          if-no-files-found: warn\n\n      - name: Retire gateway package fixtures\n        if: always()\n        run: rm -rf -- \"$SLOPSEARX_INSTALL_TEST_ROOT\"\n\n  foundation-adapters:\n    name: Foundation adapters (${{ matrix.runner }})\n    needs: foundation-prerequisites\n    runs-on: ${{ matrix.runner }}\n    timeout-minutes: 120\n    strategy:\n      fail-fast: false\n      matrix:\n        include:\n          - runner: ubuntu-24.04\n            target: x86_64-unknown-linux-gnu\n          - runner: ubuntu-24.04-arm\n            target: aarch64-unknown-linux-gnu\n    env:\n      CARGO_TERM_COLOR: always\n      RUST_BACKTRACE: 1\n    steps:\n      - uses: actions/checkout@v7\n        with:\n          fetch-depth: 0\n\n      - name: Confirm pinned toolchain\n        run: rustc --version --verbose\n\n      - name: Install mandatory offline replay isolation\n        run: |\n          sudo apt-get update\n          sudo apt-get install --yes bubblewrap apparmor\n          /usr/bin/bwrap --version\n          # Same namespace/capability flags as the native runner; no candidate runs here.\n          replay_setup_probe() {\n            /usr/bin/bwrap --unshare-user --unshare-net --unshare-pid --disable-userns \\\n              --cap-drop ALL --new-session --die-with-parent --ro-bind / / \\\n              --proc /proc --dev /dev -- /bin/sh -c \\\n              'cat /proc/self/attr/current; grep \"^CapEff:\" /proc/self/status; cat /proc/net/dev'\n          }\n          sudo cat /sys/kernel/security/apparmor/profiles | grep -E '^(bwrap|unpriv_bwrap)' || true\n          # Refuse an unexpected executable attachment instead of layering a second profile.\n          for policy in /etc/apparmor.d/*; do\n            if [ -f \"$policy\" ] && grep -Eq '^[[:space:]]*(profile[[:space:]]+[^[:space:]]+[[:space:]]+)?/usr/bin/bwrap([[:space:]]|$)' \"$policy\"; then\n              echo \"Existing bubblewrap policy: $policy\"\n              cat \"$policy\"\n              if ! grep -Eq '^[[:space:]]*profile[[:space:]]+bwrap[[:space:]]+/usr/bin/bwrap([[:space:]]|$)' \"$policy\"; then\n                echo \"Unexpected bwrap attachment identity; refusing policy replacement\" >&2\n                exit 1\n              fi\n            fi\n          done\n          if ! replay_setup_probe; then\n            sudo journalctl -k --no-pager -n 100 | grep -E 'apparmor|userns|bwrap' || true\n          fi\n          # Purpose-built bwrap setup profile plus capability-denied child stack.\n          # See docs/rust-rewrite/replay/README.md for upstream policy provenance.\n          sudo apparmor_parser --replace <<'PROFILE'\n          abi <abi/4.0>,\n          include <tunables/global>\n          profile bwrap /usr/bin/bwrap flags=(attach_disconnected,mediate_deleted) {\n            capability,\n            file rwlkm /{**,},\n            network,\n            unix,\n            ptrace,\n            signal,\n            userns,\n            mount,\n            umount,\n            pivot_root,\n            dbus,\n            pix /** -> &bwrap//&ssx_replay_unpriv,\n          }\n          profile ssx_replay_unpriv flags=(attach_disconnected,mediate_deleted) {\n            file rwlkm /{**,},\n            network,\n            unix,\n            ptrace,\n            signal,\n            userns,\n            mount,\n            umount,\n            pivot_root,\n            dbus,\n            pix /** -> &ssx_replay_unpriv,\n            audit deny capability,\n          }\n          PROFILE\n          replay_setup_probe\n\n      - name: Build pinned disposable native Valkey\n        timeout-minutes: 15\n        run: |\n          sh scripts/rust-valkey-build.sh \"$RUNNER_TEMP/ssx-valkey/bin\"\n          echo \"$RUNNER_TEMP/ssx-valkey/bin\" >> \"$GITHUB_PATH\"\n          valkey_server=\"$RUNNER_TEMP/ssx-valkey/bin/valkey-server\"\n          {\n            printf 'SLOPSEARX_TEST_VALKEY_SERVER=%s\\n' \"$valkey_server\"\n            printf 'SLOPSEARX_TEST_VALKEY_SERVER_SHA256=%s\\n' \"$(sha256sum \"$valkey_server\" | cut -d ' ' -f 1)\"\n          } >> \"$GITHUB_ENV\"\n\n      - name: Build actual-process HTTP fixture server\n        run: cargo build --locked -p ssx-http --example t02_fixture\n\n      - name: Locked workspace build\n        run: cargo build --locked --workspace --target \"${{ matrix.target }}\"\n\n      - name: Verify outbound TLS feature graph\n        run: |\n          cargo tree --locked --workspace -e features > target/rust-feature-graph.txt\n          grep -q 'rustls-platform-verifier' target/rust-feature-graph.txt\n          if grep -Eq '(^|[[:space:]])(native-tls|hyper-tls|openssl-sys)([[:space:]@v]|$)' target/rust-feature-graph.txt; then\n            echo \"unexpected native-TLS/OpenSSL backend in the locked feature graph\" >&2\n            exit 1\n          fi\n\n      - name: Fresh source-bound workspace compiler provenance\n        shell: bash\n        run: |\n          set -euo pipefail\n          umask 077\n          evidence=\"target/rust-evidence/partitions/adapters\"\n          mkdir -p \"$evidence\"\n          git ls-files -z | LC_ALL=C sort -z | xargs -0 sha256sum --zero > \"$evidence/source-before.sha256z\"\n          git ls-files --stage -z > \"$evidence/stage-before.z\"\n          [ \"$(git rev-parse HEAD)\" = \"$GITHUB_SHA\" ]\n          [ -z \"$(git status --porcelain)\" ]\n          rustc --version --verbose > \"$evidence/rustc.txt\"\n          cargo --version --verbose > \"$evidence/cargo.txt\"\n          sha256sum Cargo.toml Cargo.lock rust-toolchain.toml > \"$evidence/build-inputs.sha256\"\n          cargo build --locked --workspace --all-targets --message-format=json-render-diagnostics > \"$evidence/workspace.jsonl\"\n          PARTITION=adapters TARGET_TRIPLE=\"${{ matrix.target }}\" node <<'NODE'\n          const fs = require('node:fs'), path = require('node:path'), crypto = require('node:crypto');\n          const root = fs.realpathSync(process.cwd()), dir = `target/rust-evidence/partitions/${process.env.PARTITION}`;\n          const rows = fs.readFileSync(`${dir}/workspace.jsonl`, 'utf8').trim().split('\\n').map(JSON.parse);\n          const need = (v, s) => { if (!v) throw Error(s); };\n          need(rows.at(-1)?.reason === 'build-finished' && rows.at(-1).success === true, 'compiler completion');\n          const selected = rows.filter(r => r.reason === 'compiler-artifact' && r.executable);\n          need(selected.length > 0, 'actual executable contexts');\n          const images = selected.map(artifact => {\n            const exe = path.resolve(artifact.executable), rel = path.relative(path.join(root, 'target'), exe);\n            need(rel && !rel.startsWith('..') && !path.isAbsolute(rel), 'compiler target confinement');\n            need(artifact.filenames.includes(artifact.executable), 'declared executable filename');\n            const info = fs.lstatSync(exe); need(info.isFile() && !info.isSymbolicLink(), 'regular compiler executable');\n            return { artifact, sha256: crypto.createHash('sha256').update(fs.readFileSync(exe)).digest('hex'), bytes: info.size, mode: info.mode & 0o777 };\n          });\n          fs.writeFileSync(`${dir}/compiler-images.json`, JSON.stringify(images, null, 2) + '\\n');\n          fs.writeFileSync(`${dir}/identity.json`, JSON.stringify({ partition: process.env.PARTITION, source_revision: process.env.GITHUB_SHA, target: process.env.TARGET_TRIPLE, runner_arch: process.env.RUNNER_ARCH, run_id: process.env.GITHUB_RUN_ID, run_attempt: process.env.GITHUB_RUN_ATTEMPT, evidence_scope: 'actual pre-gate compiler contexts and executable hashes; no immutable byte-archive or OS launch-image claim' }, null, 2) + '\\n');\n          NODE\n\n","insert_utf8":""},{"at":44700,"remove_utf8":"      - name: Verify adapters source and index unchanged\n        if: always()\n        shell: bash\n        run: |\n          set -euo pipefail\n          evidence=\"target/rust-evidence/partitions/adapters\"\n          git ls-files -z | LC_ALL=C sort -z | xargs -0 sha256sum --zero > \"$evidence/source-after.sha256z\"\n          git ls-files --stage -z > \"$evidence/stage-after.z\"\n          cmp \"$evidence/source-before.sha256z\" \"$evidence/source-after.sha256z\"\n          cmp \"$evidence/stage-before.z\" \"$evidence/stage-after.z\"\n          [ \"$(git rev-parse HEAD)\" = \"$GITHUB_SHA\" ]\n          [ -z \"$(git status --porcelain)\" ]\n\n      - name: Retain adapters foundation partition evidence\n        if: always()\n        uses: actions/upload-artifact@v7\n        with:\n          name: rust-foundation-adapters-${{ matrix.target }}-${{ github.sha }}\n          path: |\n            target/rust-evidence/\n            target/rust-feature-graph.txt\n          if-no-files-found: warn\n\n  foundation:\n    name: Foundation (${{ matrix.runner }})\n    if: ${{ always() }}\n    needs: [foundation-prerequisites, foundation-runtime, foundation-adapters]\n    runs-on: ${{ matrix.runner }}\n    timeout-minutes: 10\n    strategy:\n      fail-fast: false\n      matrix:\n        include:\n          - runner: ubuntu-24.04\n            target: x86_64-unknown-linux-gnu\n          - runner: ubuntu-24.04-arm\n            target: aarch64-unknown-linux-gnu\n    env:\n      CARGO_TERM_COLOR: always\n      RUST_BACKTRACE: 1\n    steps:\n      - uses: actions/checkout@v7\n        with:\n          fetch-depth: 0\n\n      - name: Download prerequisites partition\n        id: download-prerequisites\n        if: always()\n        continue-on-error: true\n        uses: actions/download-artifact@v8\n        with:\n          name: rust-foundation-prerequisites-${{ matrix.target }}-${{ github.sha }}\n          path: ${{ runner.temp }}/foundation-partitions/prerequisites\n\n      - name: Download runtime partition\n        id: download-runtime\n        if: always()\n        continue-on-error: true\n        uses: actions/download-artifact@v8\n        with:\n          name: rust-foundation-runtime-${{ matrix.target }}-${{ github.sha }}\n          path: ${{ runner.temp }}/foundation-partitions/runtime\n\n      - name: Download adapters partition\n        id: download-adapters\n        if: always()\n        continue-on-error: true\n        uses: actions/download-artifact@v8\n        with:\n          name: rust-foundation-adapters-${{ matrix.target }}-${{ github.sha }}\n          path: ${{ runner.temp }}/foundation-partitions/adapters\n\n      - name: Merge retained partition artifacts without overwrites\n        if: always()\n        shell: bash\n        run: |\n          set -euo pipefail\n          mkdir -p target/rust-evidence\n          for partition in prerequisites runtime adapters; do\n            input=\"$RUNNER_TEMP/foundation-partitions/$partition\"\n            if [ ! -d \"$input\" ]; then continue; fi\n            if find \"$input\" -type l -print -quit | grep -q .; then\n              echo \"Refusing a symlink in retained partition artifacts\" >&2; exit 1\n            fi\n            while IFS= read -r -d '' file; do\n              rel=${file#\"$input/\"}\n              case \"$rel\" in rust-evidence/*|rust-feature-graph.txt) ;; *) echo \"Unexpected artifact path: $rel\" >&2; exit 1;; esac\n              output=\"target/$rel\"\n              if [ -e \"$output\" ]; then cmp \"$file\" \"$output\"; else mkdir -p \"$(dirname \"$output\")\"; cp \"$file\" \"$output\"; fi\n            done < <(find \"$input\" -type f -print0)\n          done\n          printf 'source_revision=%s\\ntarget=%s\\nprerequisites=%s\\nruntime=%s\\nadapters=%s\\n' \"$GITHUB_SHA\" \"${{ matrix.target }}\" \"${{ needs.foundation-prerequisites.result }}\" \"${{ needs.foundation-runtime.result }}\" \"${{ needs.foundation-adapters.result }}\" > target/rust-evidence/foundation-partitions.txt\n\n","insert_utf8":""},{"at":49953,"remove_utf8":"            target/rust-evidence/T02.json\n","insert_utf8":""},{"at":50787,"remove_utf8":"            target/rust-evidence/partitions/\n            target/rust-evidence/foundation-partitions.txt\n","insert_utf8":""},{"at":50926,"remove_utf8":"      - name: Require every partition and the complete original evidence\n","insert_utf8":"      - name: Retire gateway package fixtures\n"},{"at":51020,"remove_utf8":"        shell: bash\n        run: |\n          set -euo pipefail\n          [ \"${{ needs.foundation-prerequisites.result }}\" = success ]\n          [ \"${{ needs.foundation-runtime.result }}\" = success ]\n          [ \"${{ needs.foundation-adapters.result }}\" = success ]\n          [ \"${{ steps.download-prerequisites.outcome }}\" = success ]\n          [ \"${{ steps.download-runtime.outcome }}\" = success ]\n          [ \"${{ steps.download-adapters.outcome }}\" = success ]\n          for plan in F03 F04 T04 T05 T06 T07 T01 F05 F06 F09 F12 F11 F13 F14 F15 T02 F07 W00 W01 W02 W03 W04 W05 W11 W06 O03 O01 F08 F10 T09 A01 A02 A03 A04 A05 A06 A07 A08 A09 A10 A11 A13 A14 A12; do test -s \"target/rust-evidence/$plan.json\"; done\n          test -s target/rust-feature-graph.txt\n          for partition in prerequisites runtime adapters; do\n            test -s \"target/rust-evidence/partitions/$partition/workspace.jsonl\"\n            test -s \"target/rust-evidence/partitions/$partition/compiler-images.json\"\n            cmp \"target/rust-evidence/partitions/$partition/source-before.sha256z\" \"target/rust-evidence/partitions/$partition/source-after.sha256z\"\n            cmp \"target/rust-evidence/partitions/$partition/stage-before.z\" \"target/rust-evidence/partitions/$partition/stage-after.z\"\n          done\n          cmp target/rust-evidence/partitions/prerequisites/source-before.sha256z target/rust-evidence/partitions/runtime/source-before.sha256z\n          cmp target/rust-evidence/partitions/prerequisites/source-before.sha256z target/rust-evidence/partitions/adapters/source-before.sha256z\n          TARGET_TRIPLE=\"${{ matrix.target }}\" node <<'NODE'\n          const fs = require('node:fs');\n          for (const partition of ['prerequisites', 'runtime', 'adapters']) {\n            const value = JSON.parse(fs.readFileSync(`target/rust-evidence/partitions/${partition}/identity.json`, 'utf8'));\n            if (value.partition !== partition || value.source_revision !== process.env.GITHUB_SHA || value.target !== process.env.TARGET_TRIPLE || value.run_id !== process.env.GITHUB_RUN_ID || value.run_attempt !== process.env.GITHUB_RUN_ATTEMPT) throw Error('partition identity mismatch');\n          }\n          NODE\n          shopt -s nullglob\n          controls=(target/rust-evidence/A10-linux-control/\"${{ github.run_id }}-${{ github.run_attempt }}-${{ runner.arch }}\".*/control-result.txt)\n          [ \"${#controls[@]}\" -eq 1 ]\n          grep -Fxq EXPECTED_SEMANTIC_FAILURE_CONFIRMED \"${controls[0]}\"\n\n","insert_utf8":"        run: rm -rf -- \"$SLOPSEARX_INSTALL_TEST_ROOT\"\n"}]"###,
        )),
        ".pre-commit-config.yaml" => Ok((
            json!({"path":".pre-commit-config.yaml","bytes":12547,"sha256":"243ccd061ac4d1739e59ccddbc1cf0e88c832695a8df463305712bfdd75a8000","git_blob":"8b9e14394e51ceb9d6b8c050587adf43198f8c89","mode":"0644"}),
            12910,
            "ea4cc40b3aed49442d4cdc110420b29c997031101752e33280ea900780d4feb1",
            "3d38d47e2bc99205532651979fc3d74faa89fe63091d74071b8677d029f79539",
            r###"[{"at":3165,"remove_utf8":"        exclude: ^(docs/experiments/evidence/(.*\\.(json|txt)|EXP-003/reproduce\\.md)|crates/xtask/fixtures/f13/native-record\\.json|docs/rust-rewrite/t06/validation\\.json)$|^(?:crates/ssx\\-domain/tests/fixtures/a05_previous_media_sanitizer\\.rs|crates/ssx\\-source\\-codecs/MAPPING\\-SOURCE\\-NOTICES\\.txt|crates/ssx\\-source\\-codecs/MAPPING\\-UPSTREAM\\-NOTICES\\.txt|crates/ssx\\-source\\-codecs/MAPPING\\-UPSTREAM\\-SINGLEBYTE\\-NOTICES\\.txt|crates/ssx\\-source\\-codecs/PYTHON\\-SOURCE\\-NOTICES\\.txt|crates/ssx\\-source\\-codecs/UNICODE\\-SOURCE\\-HEADERS\\.txt|crates/ssx\\-source\\-codecs/provenance/idna/RFC3454\\-ORIGINAL\\-NOTICE\\.txt|crates/ssx\\-source\\-codecs/provenance/idna/UNICODE\\-3\\.2\\-ORIGINAL\\-NOTICE\\.html|crates/ssx\\-source\\-codecs/provenance/idna/UnicodeCharacterDatabase\\-3\\.2\\.0\\.html|crates/ssx\\-source\\-codecs/provenance/idna/idna\\.py|crates/ssx\\-source\\-codecs/provenance/idna/inputs/CompositionExclusions\\-3\\.2\\.0\\.txt|crates/ssx\\-source\\-codecs/provenance/idna/inputs/SpecialCasing\\-16\\.0\\.0\\.txt|crates/ssx\\-source\\-codecs/provenance/idna/inputs/aliases\\.py|crates/ssx\\-source\\-codecs/provenance/idna/inputs/stringprep\\.py|crates/ssx\\-source\\-codecs/provenance/idna/mkstringprep\\.py|crates/ssx\\-source\\-codecs/provenance/idna/rfc3454\\.txt|crates/ssx\\-source\\-codecs/provenance/singlebyte\\-icu/koi8\\-t\\.ucm|docs/rust\\-rewrite/a05/libxml\\-safe\\-api\\-proposal\\.patch|docs/rust\\-rewrite/a05/surrogate\\-iconv\\-version\\-comparison\\.json|vendor/libxml/\\.cargo_vcs_info\\.json|vendor/libxml/\\.rustfmt\\.toml|vendor/libxml/CHANGELOG\\.md|vendor/libxml/README\\.md|vendor/libxml/appveyor\\.yml|vendor/libxml/src/wrapper\\.h|vendor/libxml/tests/VALGRIND\\.md|vendor/libxml/tests/resources/canonical_1_1/3_1_input\\.xml|vendor/libxml/tests/resources/canonical_1_1/3_1_output\\.xml|vendor/libxml/tests/resources/canonical_1_1/3_1_output_no_comment\\.xml|vendor/libxml/tests/resources/canonical_1_1/3_2_input\\.xml|vendor/libxml/tests/resources/canonical_1_1/Readme\\.md|vendor/libxml/tests/resources/canonical_exclusive/1_input\\.xml|vendor/libxml/tests/resources/canonical_exclusive/1_output\\.xml|vendor/libxml/tests/resources/example\\.html|vendor/libxml/tests/resources/file01_utf16le\\.xml|vendor/libxml/tests/resources/file01_utf16le_bom\\.xml|vendor/libxml/tests/resources/ids\\.xml|vendor/libxml/tests/resources/staff\\.xml|vendor/libxml/tests/resources/unformatted\\.xml|vendor/libxml2/a05\\-normal\\-data\\-nul\\.patch)$|^(?:contracts/unicode/python-text-source/Objects__unicodectype\\.c\\.txt|contracts/unicode/python-text-source/Objects__unicodetype_db\\.h\\.txt|contracts/unicode/python-text-source/Modules__unicodedata_db\\.h\\.txt)$|^docs/rust-rewrite/a05/source-amendments/3f1b9cca-cold-bootstrap\\.rs\\.frozen$|^docs/rust-rewrite/a07/qualification/pubchem-owner-clock-negative\\.stdout$|^logs/a05-codec-donor-final-alias-regression\\.log$|^(?:docs/rust\\-rewrite/t07/source\\-inputs/CPYTHON\\-unicodectype\\.c\\.txt|docs/rust\\-rewrite/t07/source\\-inputs/CPYTHON\\-unicodetype_db\\.h\\.txt|docs/rust\\-rewrite/t07/source\\-inputs/fastmcp\\-4\\.0\\.11\\.dist\\-info__LICENSE\\.txt|vendor/ssx\\-http\\-source/httparse/\\.cargo_vcs_info\\.json|vendor/ssx\\-http\\-source/httparse/LICENSE\\-MIT|vendor/ssx\\-http\\-source/hyper\\-util/\\.cargo_vcs_info\\.json|vendor/ssx\\-http\\-source/hyper/\\.cargo_vcs_info\\.json|vendor/ssx\\-http\\-source/reqwest/\\.cargo_vcs_info\\.json|vendor/ssx\\-http\\-source/reqwest/LICENSE\\-MIT|vendor/ssx\\-http\\-source/reqwest/src/source_pool/python\\-text\\-source/Modules__unicodedata_db\\.h\\.txt|vendor/ssx\\-http\\-source/reqwest/src/source_pool/python\\-text\\-source/Objects__unicodectype\\.c\\.txt|vendor/ssx\\-http\\-source/reqwest/src/source_pool/python\\-text\\-source/Objects__unicodetype_db\\.h\\.txt|vendor/ssx\\-sdk\\-auth/patches/rmcp\\-native\\-dev\\-fixtures\\.patch|vendor/ssx\\-sdk\\-auth/rmcp/\\.cargo_vcs_info\\.json|vendor/ssx\\-sdk\\-auth/rmcp/src/handler/server/resource\\.rs|vendor/ssx\\-sdk\\-auth/rmcp/tests/test_deserialization/tool_list_result\\.json|vendor/ssx\\-sdk\\-auth/rmcp/tests/test_message_schema/client_json_rpc_message_schema\\.json|vendor/ssx\\-sdk\\-auth/rmcp/tests/test_message_schema/client_json_rpc_message_schema_current\\.json|vendor/ssx\\-sdk\\-auth/rmcp/tests/test_message_schema/server_json_rpc_message_schema\\.json|vendor/ssx\\-sdk\\-auth/rmcp/tests/test_message_schema/server_json_rpc_message_schema_current\\.json|vendor/ssx\\-sdk\\-auth/rmcp/tests/test_tool_handler\\.rs|vendor/ssx\\-sdk\\-auth/rmcp/tests/test_with_js/\\.gitignore|vendor/ssx\\-sdk\\-auth/rmcp/tests/test_with_js/client\\.js|vendor/ssx\\-sdk\\-auth/rmcp/tests/test_with_js/server\\.js|vendor/ssx\\-sdk\\-auth/rmcp/tests/test_with_js/streamable_server\\.js)$|^(?:crates/ssx\\-source\\-codecs/provenance/idna2008/inputs/Objects__unicodetype_db\\.h\\.txt|crates/ssx\\-source\\-codecs/provenance/idna2008/inputs/unicodedata_db\\.h)$\n","insert_utf8":"        exclude: ^(docs/experiments/evidence/(.*\\.(json|txt)|EXP-003/reproduce\\.md)|crates/xtask/fixtures/f13/native-record\\.json|docs/rust-rewrite/t06/validation\\.json)$|^(?:crates/ssx\\-domain/tests/fixtures/a05_previous_media_sanitizer\\.rs|crates/ssx\\-source\\-codecs/MAPPING\\-SOURCE\\-NOTICES\\.txt|crates/ssx\\-source\\-codecs/MAPPING\\-UPSTREAM\\-NOTICES\\.txt|crates/ssx\\-source\\-codecs/MAPPING\\-UPSTREAM\\-SINGLEBYTE\\-NOTICES\\.txt|crates/ssx\\-source\\-codecs/PYTHON\\-SOURCE\\-NOTICES\\.txt|crates/ssx\\-source\\-codecs/UNICODE\\-SOURCE\\-HEADERS\\.txt|crates/ssx\\-source\\-codecs/provenance/idna/RFC3454\\-ORIGINAL\\-NOTICE\\.txt|crates/ssx\\-source\\-codecs/provenance/idna/UNICODE\\-3\\.2\\-ORIGINAL\\-NOTICE\\.html|crates/ssx\\-source\\-codecs/provenance/idna/UnicodeCharacterDatabase\\-3\\.2\\.0\\.html|crates/ssx\\-source\\-codecs/provenance/idna/idna\\.py|crates/ssx\\-source\\-codecs/provenance/idna/inputs/CompositionExclusions\\-3\\.2\\.0\\.txt|crates/ssx\\-source\\-codecs/provenance/idna/inputs/SpecialCasing\\-16\\.0\\.0\\.txt|crates/ssx\\-source\\-codecs/provenance/idna/inputs/aliases\\.py|crates/ssx\\-source\\-codecs/provenance/idna/inputs/stringprep\\.py|crates/ssx\\-source\\-codecs/provenance/idna/mkstringprep\\.py|crates/ssx\\-source\\-codecs/provenance/idna/rfc3454\\.txt|crates/ssx\\-source\\-codecs/provenance/singlebyte\\-icu/koi8\\-t\\.ucm|docs/rust\\-rewrite/a05/libxml\\-safe\\-api\\-proposal\\.patch|docs/rust\\-rewrite/a05/surrogate\\-iconv\\-version\\-comparison\\.json|vendor/libxml/\\.cargo_vcs_info\\.json|vendor/libxml/\\.rustfmt\\.toml|vendor/libxml/CHANGELOG\\.md|vendor/libxml/README\\.md|vendor/libxml/appveyor\\.yml|vendor/libxml/src/wrapper\\.h|vendor/libxml/tests/VALGRIND\\.md|vendor/libxml/tests/resources/canonical_1_1/3_1_input\\.xml|vendor/libxml/tests/resources/canonical_1_1/3_1_output\\.xml|vendor/libxml/tests/resources/canonical_1_1/3_1_output_no_comment\\.xml|vendor/libxml/tests/resources/canonical_1_1/3_2_input\\.xml|vendor/libxml/tests/resources/canonical_1_1/Readme\\.md|vendor/libxml/tests/resources/canonical_exclusive/1_input\\.xml|vendor/libxml/tests/resources/canonical_exclusive/1_output\\.xml|vendor/libxml/tests/resources/example\\.html|vendor/libxml/tests/resources/file01_utf16le\\.xml|vendor/libxml/tests/resources/file01_utf16le_bom\\.xml|vendor/libxml/tests/resources/ids\\.xml|vendor/libxml/tests/resources/staff\\.xml|vendor/libxml/tests/resources/unformatted\\.xml|vendor/libxml2/a05\\-normal\\-data\\-nul\\.patch)$|^(?:contracts/unicode/python-text-source/Objects__unicodectype\\.c\\.txt|contracts/unicode/python-text-source/Objects__unicodetype_db\\.h\\.txt|contracts/unicode/python-text-source/Modules__unicodedata_db\\.h\\.txt)$|^docs/rust-rewrite/a05/source-amendments/3f1b9cca-cold-bootstrap\\.rs\\.frozen$|^docs/rust-rewrite/a07/qualification/pubchem-owner-clock-negative\\.stdout$|^logs/a05-codec-donor-final-alias-regression\\.log$|^(?:docs/rust\\-rewrite/t07/source\\-inputs/CPYTHON\\-unicodectype\\.c\\.txt|docs/rust\\-rewrite/t07/source\\-inputs/CPYTHON\\-unicodetype_db\\.h\\.txt|docs/rust\\-rewrite/t07/source\\-inputs/fastmcp\\-4\\.0\\.11\\.dist\\-info__LICENSE\\.txt|vendor/ssx\\-http\\-source/httparse/\\.cargo_vcs_info\\.json|vendor/ssx\\-http\\-source/httparse/LICENSE\\-MIT|vendor/ssx\\-http\\-source/hyper\\-util/\\.cargo_vcs_info\\.json|vendor/ssx\\-http\\-source/hyper/\\.cargo_vcs_info\\.json|vendor/ssx\\-http\\-source/reqwest/\\.cargo_vcs_info\\.json|vendor/ssx\\-http\\-source/reqwest/LICENSE\\-MIT|vendor/ssx\\-http\\-source/reqwest/src/source_pool/python\\-text\\-source/Modules__unicodedata_db\\.h\\.txt|vendor/ssx\\-http\\-source/reqwest/src/source_pool/python\\-text\\-source/Objects__unicodectype\\.c\\.txt|vendor/ssx\\-http\\-source/reqwest/src/source_pool/python\\-text\\-source/Objects__unicodetype_db\\.h\\.txt|vendor/ssx\\-sdk\\-auth/patches/rmcp\\-native\\-dev\\-fixtures\\.patch|vendor/ssx\\-sdk\\-auth/rmcp/\\.cargo_vcs_info\\.json|vendor/ssx\\-sdk\\-auth/rmcp/src/handler/server/resource\\.rs|vendor/ssx\\-sdk\\-auth/rmcp/tests/test_deserialization/tool_list_result\\.json|vendor/ssx\\-sdk\\-auth/rmcp/tests/test_message_schema/client_json_rpc_message_schema\\.json|vendor/ssx\\-sdk\\-auth/rmcp/tests/test_message_schema/client_json_rpc_message_schema_current\\.json|vendor/ssx\\-sdk\\-auth/rmcp/tests/test_message_schema/server_json_rpc_message_schema\\.json|vendor/ssx\\-sdk\\-auth/rmcp/tests/test_message_schema/server_json_rpc_message_schema_current\\.json|vendor/ssx\\-sdk\\-auth/rmcp/tests/test_tool_handler\\.rs|vendor/ssx\\-sdk\\-auth/rmcp/tests/test_with_js/\\.gitignore|vendor/ssx\\-sdk\\-auth/rmcp/tests/test_with_js/client\\.js|vendor/ssx\\-sdk\\-auth/rmcp/tests/test_with_js/server\\.js|vendor/ssx\\-sdk\\-auth/rmcp/tests/test_with_js/streamable_server\\.js)$\n"},{"at":10473,"remove_utf8":"        exclude: ^docs/rust-rewrite/t06/(entities|validation)\\.json$|^docs/rust-rewrite/w06/(source\\.json|evidence/source-before-(composite-id-correction|unique-snapshot-ids)\\.json|evidence/review-public-source-outcomes\\.json)$|^docs/rust-rewrite/a10/(source-boundaries-v2\\.json|native-differences-v1\\.json)$|^docs/rust-rewrite/a11/(source-boundaries-v2|native-differences-v1)\\.json$|^(?:contracts/unicode/python-text-source/Modules__unicodedata_db\\.h\\.txt)$|^(?:crates/ssx\\-source\\-codecs/provenance/idna2008/inputs/unicodedata_db\\.h|crates/ssx\\-source\\-codecs/provenance/idna2008/inputs/unicodename_db\\.h|crates/ssx\\-source\\-codecs/src/idna2008_data\\.rs)$\n","insert_utf8":"        exclude: ^docs/rust-rewrite/t06/(entities|validation)\\.json$|^docs/rust-rewrite/w06/(source\\.json|evidence/source-before-(composite-id-correction|unique-snapshot-ids)\\.json|evidence/review-public-source-outcomes\\.json)$|^docs/rust-rewrite/a10/(source-boundaries-v2\\.json|native-differences-v1\\.json)$|^docs/rust-rewrite/a11/(source-boundaries-v2|native-differences-v1)\\.json$|^(?:contracts/unicode/python-text-source/Modules__unicodedata_db\\.h\\.txt)$\n"}]"###,
        )),
        _ => Err("T07 unit-route unlisted configuration caller".into()),
    }
}
// Exact runner.temp context repair. Only these four spans reconstruct the
// accepted e511 workflow operand; the older capsule inverse stays unchanged.
const WORKFLOW_ENV_INVERSE: &str = r###"[{"at":569,"remove_utf8":"","insert_utf8":"      SLOPSEARX_INSTALL_TEST_ROOT: ${{ runner.temp }}/ssx-gateway-install\n      SLOPSEARX_T07_UNIT_TEST_ROOT: ${{ runner.temp }}/ssx-t07-unit-tests\n"},{"at":850,"remove_utf8":"\n      - name: Configure private gateway fixture roots\n        env:\n          SLOPSEARX_INSTALL_TEST_ROOT: ${{ runner.temp }}/ssx-gateway-install\n          SLOPSEARX_T07_UNIT_TEST_ROOT: ${{ runner.temp }}/ssx-t07-unit-tests\n        run: |\n          printf 'SLOPSEARX_INSTALL_TEST_ROOT=%s\\n' \"$SLOPSEARX_INSTALL_TEST_ROOT\" >> \"$GITHUB_ENV\"\n          printf 'SLOPSEARX_T07_UNIT_TEST_ROOT=%s\\n' \"$SLOPSEARX_T07_UNIT_TEST_ROOT\" >> \"$GITHUB_ENV\"\n","insert_utf8":""},{"at":20995,"remove_utf8":"","insert_utf8":"      SLOPSEARX_INSTALL_TEST_ROOT: ${{ runner.temp }}/ssx-gateway-install\n      SLOPSEARX_T07_UNIT_TEST_ROOT: ${{ runner.temp }}/ssx-t07-unit-tests\n"},{"at":21079,"remove_utf8":"\n      - name: Configure private gateway fixture roots\n        env:\n          SLOPSEARX_INSTALL_TEST_ROOT: ${{ runner.temp }}/ssx-gateway-install\n          SLOPSEARX_T07_UNIT_TEST_ROOT: ${{ runner.temp }}/ssx-t07-unit-tests\n        run: |\n          printf 'SLOPSEARX_INSTALL_TEST_ROOT=%s\\n' \"$SLOPSEARX_INSTALL_TEST_ROOT\" >> \"$GITHUB_ENV\"\n          printf 'SLOPSEARX_T07_UNIT_TEST_ROOT=%s\\n' \"$SLOPSEARX_T07_UNIT_TEST_ROOT\" >> \"$GITHUB_ENV\"\n","insert_utf8":""}]"###;
// Exact shipping-prerequisite profile and runtime debug0-only profile; tests and deadlines are unchanged.
const WORKFLOW_PROFILE_INVERSE: &str = r###"[{"at":545,"remove_utf8":"      CARGO_BUILD_JOBS: 2\n      CARGO_PROFILE_DEV_DEBUG: 0\n      CARGO_PROFILE_TEST_DEBUG: 0\n      CARGO_INCREMENTAL: 0\n","insert_utf8":""},{"at":21091,"remove_utf8":"      CARGO_PROFILE_DEV_DEBUG: 0\n      CARGO_PROFILE_TEST_DEBUG: 0\n","insert_utf8":""}]"###;
// Exact compiler-artifact smoke capture continuation; old accepted workflow inverses stay unchanged.
// Exact normal-main W08/scheduling union before the accepted smoke operand.
const WORKFLOW_MAIN_JOIN_INVERSE: &str = r#####"[{"at":34336,"remove_utf8":"      - name: Native dependency dossier and policy-checked composition evidence\n        run: cargo xtask check W08 --output target/rust-evidence/W08.json\n","insert_utf8":""},{"at":51885,"remove_utf8":"            target/rust-evidence/W08.json\n","insert_utf8":""},{"at":53794,"remove_utf8":"          for plan in F03 F04 T04 T05 T06 T07 T01 F05 F06 F09 F12 F11 F13 F14 F15 T02 F07 W00 W01 W02 W03 W04 W05 W11 W06 W08 O03 O01 F08 F10 T09 A01 A02 A03 A04 A05 A06 A07 A08 A09 A10 A11 A13 A14 A12; do test -s \"target/rust-evidence/$plan.json\"; done\n","insert_utf8":"          for plan in F03 F04 T04 T05 T06 T07 T01 F05 F06 F09 F12 F11 F13 F14 F15 T02 F07 W00 W01 W02 W03 W04 W05 W11 W06 O03 O01 F08 F10 T09 A01 A02 A03 A04 A05 A06 A07 A08 A09 A10 A11 A13 A14 A12; do test -s \"target/rust-evidence/$plan.json\"; done\n"}]"#####;
fn workflow_main_join_claim() -> Value {
    json!({"path":".github/workflows/rust.yml","bytes":56270,"sha256":"49beb3bf98a511e82e24ba314ea2d3490de079118feefad83bc81ea98b78d2a8","git_blob":"4974dcf8c871ed665f85f167deffc57cc43bb921","mode":"0644"})
}
fn workflow_main_join_frame(
    expected: &Value,
    physical: &[u8],
    mode: &str,
    inverse: &Value,
) -> Result<Vec<u8>, String> {
    need(
        *expected == workflow_main_join_claim(),
        "T07 normal-main workflow exact caller differs",
    )?;
    material(expected, physical, mode)?;
    need(
        digest(&serde_json::to_vec(inverse).map_err(|e| e.to_string())?)
            == "09e92bd4f0031ac8fa7ef5cfb7ac2ade2ab082291a702368df161e6071416abd",
        "T07 normal-main workflow finite inverse differs",
    )?;
    let prior = edits(physical, inverse)?;
    material(&workflow_smoke_claim(), &prior, "0644")?;
    Ok(prior)
}
#[cfg(test)]
fn workflow_main_join_controls(physical: &[u8]) -> Result<Vec<u8>, String> {
    let claim = workflow_main_join_claim();
    let inverse: Value =
        serde_json::from_str(WORKFLOW_MAIN_JOIN_INVERSE).map_err(|e| e.to_string())?;
    let prior = workflow_main_join_frame(&claim, physical, "0644", &inverse)?;
    need(
        workflow_main_join_frame(&claim, &prior, "0644", &inverse).is_err(),
        "T07 normal-main workflow old-as-current accepted",
    )?;
    need(
        workflow_main_join_frame(&claim, physical, "0600", &inverse).is_err(),
        "T07 normal-main workflow wrong mode accepted",
    )?;
    let mut changed = physical.to_vec();
    changed.push(0);
    need(
        workflow_main_join_frame(&claim, &changed, "0644", &inverse).is_err(),
        "T07 normal-main workflow changed current accepted",
    )?;
    for key in ["path", "bytes", "sha256", "git_blob", "mode"] {
        let mut wrong = claim.clone();
        wrong[key] = json!("wrong");
        need(
            workflow_main_join_frame(&wrong, physical, "0644", &inverse).is_err(),
            "T07 normal-main workflow wrong caller accepted",
        )?;
    }
    for selected in 0..3 {
        for key in ["at", "remove_utf8", "insert_utf8"] {
            let mut wrong = inverse.clone();
            wrong[selected][key] = json!("wrong");
            need(
                workflow_main_join_frame(&claim, physical, "0644", &wrong).is_err(),
                "T07 normal-main workflow changed span accepted",
            )?;
        }
    }
    let mut missing = inverse.clone();
    missing.as_array_mut().unwrap().pop();
    let mut extra = inverse.clone();
    extra.as_array_mut().unwrap().push(inverse[0].clone());
    for wrong in [missing, extra] {
        need(
            workflow_main_join_frame(&claim, physical, "0644", &wrong).is_err(),
            "T07 normal-main workflow missing/duplicate span accepted",
        )?;
    }
    Ok(prior)
}

const WORKFLOW_SMOKE_INVERSE: &str = r###"[{"at":40123,"insert_utf8":"      - name: Locked workspace build\n        run: cargo build --locked --workspace --target \"${{ matrix.target }}\"\n","remove_utf8":"      - name: Locked workspace build\n        run: |\n          set -eu\n          evidence=\"target/rust-evidence/partitions/adapters\"\n          mkdir -p \"$evidence\"\n          cargo build --locked -p xtask --example foundation_python_boundary --target \"${{ matrix.target }}\"\n          cargo test --locked -p xtask --example foundation_python_boundary --target \"${{ matrix.target }}\" -- --test-threads=1\n          producer_exit=0\n          cargo build --locked --workspace --target \"${{ matrix.target }}\" --message-format=json-render-diagnostics > \"$evidence/native-workspace.jsonl\" || producer_exit=$?\n          printf '%s\\n' \"$producer_exit\" > \"$evidence/native-workspace.exit\"\n          [ \"$producer_exit\" -eq 0 ]\n          \"target/${{ matrix.target }}/debug/examples/foundation_python_boundary\" \"$PWD\" \"$PWD/target\" \"${{ matrix.target }}\" \"$evidence/native-workspace.jsonl\" \"$evidence/native-workspace.exit\" \"$PWD/target/${{ matrix.target }}/debug/ssx-server\" --record > \"$evidence/native-build-boundary.json\"\n"},{"at":46246,"insert_utf8":"","remove_utf8":"          SSX_FOUNDATION_BUILD_JSON: target/rust-evidence/partitions/adapters/native-workspace.jsonl\n          SSX_FOUNDATION_BUILD_EXIT: target/rust-evidence/partitions/adapters/native-workspace.exit\n          SSX_FOUNDATION_TARGET: ${{ matrix.target }}\n          SSX_FOUNDATION_TARGET_ROOT: target\n          SSX_FOUNDATION_BUILD_BOUNDARY: target/rust-evidence/partitions/adapters/native-build-boundary.json\n          SSX_FOUNDATION_BOUNDARY_BIN: target/${{ matrix.target }}/debug/examples/foundation_python_boundary\n"}]"###;
fn workflow_smoke_claim() -> Value {
    json!({"path":".github/workflows/rust.yml","bytes":56070,"sha256":"372916cb8508d7e58b37659c72bd1d1ef763222c7aee19e7ba98c7ed3a3f1fab","git_blob":"9ed1b09ec02ca27fe35239ecb46f78ee8da8a892","mode":"0644"})
}
fn workflow_smoke_frame(
    expected: &Value,
    physical: &[u8],
    mode: &str,
    inverse: &Value,
) -> Result<Vec<u8>, String> {
    need(
        *expected == workflow_smoke_claim(),
        "T07 smoke exact current caller differs",
    )?;
    material(expected, physical, mode)?;
    need(
        digest(&serde_json::to_vec(inverse).map_err(|e| e.to_string())?)
            == "c403f0be99a0e11efad2c8df41d77a362a0faefca2f2aa32f87c909297698fed",
        "T07 smoke closed two-span inverse differs",
    )?;
    let prior = edits(physical, inverse)?;
    material(&workflow_env_claim(), &prior, "0644")?;
    Ok(prior)
}
#[cfg(test)]
fn workflow_smoke_controls(physical: &[u8]) -> Result<Vec<u8>, String> {
    let claim = workflow_smoke_claim();
    let inverse: Value = serde_json::from_str(WORKFLOW_SMOKE_INVERSE).map_err(|e| e.to_string())?;
    let prior = workflow_smoke_frame(&claim, physical, "0644", &inverse)?;
    need(
        workflow_smoke_frame(&claim, &prior, "0644", &inverse).is_err(),
        "T07 smoke old-as-current accepted",
    )?;
    need(
        workflow_smoke_frame(&claim, physical, "0600", &inverse).is_err(),
        "T07 smoke wrong mode accepted",
    )?;
    let mut changed = physical.to_vec();
    changed.push(0);
    need(
        workflow_smoke_frame(&claim, &changed, "0644", &inverse).is_err(),
        "T07 smoke changed current accepted",
    )?;
    for key in ["path", "bytes", "sha256", "git_blob", "mode"] {
        let mut wrong = claim.clone();
        wrong[key] = json!("wrong");
        need(
            workflow_smoke_frame(&wrong, physical, "0644", &inverse).is_err(),
            "T07 smoke wrong caller accepted",
        )?;
    }
    for selected in 0..2 {
        for key in ["at", "remove_utf8", "insert_utf8"] {
            let mut wrong = inverse.clone();
            wrong[selected][key] = json!("wrong");
            need(
                workflow_smoke_frame(&claim, physical, "0644", &wrong).is_err(),
                "T07 smoke modified span accepted",
            )?;
        }
    }
    let mut missing = inverse.clone();
    missing.as_array_mut().unwrap().pop();
    let mut extra = inverse.clone();
    extra.as_array_mut().unwrap().push(inverse[0].clone());
    for wrong in [missing, extra] {
        need(
            workflow_smoke_frame(&claim, physical, "0644", &wrong).is_err(),
            "T07 smoke missing/duplicate span accepted",
        )?;
    }
    Ok(prior)
}
fn workflow_env_claim() -> Value {
    json!({"bytes":54657,"git_blob":"88c8316361219d8a49261e955d285e7da1afeba3","mode":"0644","path":".github/workflows/rust.yml","sha256":"cac3e563ffb7a9b8d1bb83d40e992c585cbe0f7b7a93bfdbf951b6127a839e7a"})
}
// Only the two accepted producer scheduling edges reconstruct the prior workflow.
const WORKFLOW_CONCURRENCY_INVERSE: &str = r###"[{"at":20727,"remove_utf8":"","insert_utf8":"    needs: foundation-prerequisites\n"},{"at":36311,"remove_utf8":"","insert_utf8":"    needs: foundation-prerequisites\n"}]"###;
fn workflow_concurrency_frame(
    expected: &Value,
    physical: &[u8],
    mode: &str,
    inverse: &Value,
) -> Result<Vec<u8>, String> {
    need(
        *expected == workflow_env_claim(),
        "T07 workflow concurrency exact current caller differs",
    )?;
    material(expected, physical, mode)?;
    need(
        digest(&serde_json::to_vec(inverse).map_err(|e| e.to_string())?)
            == "2aca024bc8680336a8c999b8bf717206c4acdf877c22d8724cf18df5b5e22c6f",
        "T07 workflow concurrency closed two-span inverse differs",
    )?;
    let prior = edits(physical, inverse)?;
    material(
        &json!({"bytes":54729,"git_blob":"3a891815bacaef9c6e7641b8fbc409a17977680a","mode":"0644","path":".github/workflows/rust.yml","sha256":"77be83db798fa612007a608718d90f24c03deda36944de34f346c3a9c8cf76ad"}),
        &prior,
        "0644",
    )?;
    Ok(prior)
}
fn workflow_env_frame(
    expected: &Value,
    physical: &[u8],
    mode: &str,
    inverse: &Value,
) -> Result<Vec<u8>, String> {
    need(
        *expected == workflow_env_claim(),
        "T07 workflow env exact current caller differs",
    )?;
    material(expected, physical, mode)?;
    let concurrency_inverse: Value =
        serde_json::from_str(WORKFLOW_CONCURRENCY_INVERSE).map_err(|e| e.to_string())?;
    let concurrency_prior =
        workflow_concurrency_frame(expected, physical, mode, &concurrency_inverse)?;
    let physical = concurrency_prior.as_slice();
    let profile_inverse: Value =
        serde_json::from_str(WORKFLOW_PROFILE_INVERSE).map_err(|e| e.to_string())?;
    need(
        digest(&serde_json::to_vec(&profile_inverse).map_err(|e| e.to_string())?)
            == "f8e6b8a17efbe18c3cdc503055b2e54e7e809bcbe8144fc4884cb263f3ce6b7f",
        "T07 workflow profile closed two-span inverse differs",
    )?;
    let accepted = edits(physical, &profile_inverse)?;
    material(
        &json!({"bytes":54542,"git_blob":"8fcc18490bc846f66ef4a6aff3989760cb20a980","mode":"0644","path":".github/workflows/rust.yml","sha256":"3f4ba10fed18916af8fc390c2e755a6a3c144f014d561d03c103cd674df542c3"}),
        &accepted,
        "0644",
    )?;
    let physical = accepted.as_slice();
    need(
        digest(&serde_json::to_vec(inverse).map_err(|e| e.to_string())?)
            == "2c9c1976e655cd84e1162b232a30704cde75e6911ab224b87c9988206306da45",
        "T07 workflow env closed four-span inverse differs",
    )?;
    let prior = edits(physical, inverse)?;
    material(
        &json!({"bytes":53956,"git_blob":"e35aa9d7014ef2b3d682ca40a38b9ea31050dd03","mode":"0644","path":".github/workflows/rust.yml","sha256":"e511701afc001bbd3eb160f16cc029b7ed2a0e1f73f6d5953ad056cb21c10937"}),
        &prior,
        "0644",
    )?;
    Ok(prior)
}
// The combined CLI/gateway source is authenticated before the accepted965
// configuration operand enters any unchanged historical workflow/hook proof.
fn stacked_config_spec(
    path: &str,
) -> Result<(Value, Value, usize, &'static str, &'static str), String> {
    match path {
        ".github/workflows/rust.yml" => Ok((
            json!({"path":".github/workflows/rust.yml","bytes":68228,"sha256":"0e51efcf055712ef248476a5cb292f52566e2809c9f9f860ed53851799451b58","git_blob":"71743ac9ebd564b71c46fa75f4ea8816de8d8093","mode":"0644"}),
            json!({"path":".github/workflows/rust.yml","bytes":56270,"sha256":"49beb3bf98a511e82e24ba314ea2d3490de079118feefad83bc81ea98b78d2a8","git_blob":"4974dcf8c871ed665f85f167deffc57cc43bb921","mode":"0644"}),
            28,
            "76c57cc46914c65621894f3d8427ac12aad2250f4170a737b7bdac0f7cc1f47a",
            r#######"[{"at":5507,"insert_utf8":"          cargo build --locked --workspace --all-targets --message-format=json-render-diagnostics > \"$evidence/workspace.jsonl\"\n","remove_utf8":"          cargo build --locked --workspace --all-targets --features ssx-cli-hyper/full,ssx-cli-hyper/source-client-framing --message-format=json-render-diagnostics > \"$evidence/workspace.jsonl\"\n"},{"at":6294,"insert_utf8":"","remove_utf8":"          const privateUnit = rows.filter(r => r.reason === 'compiler-artifact' && r.manifest_path === path.join(root, 'vendor/ssx-cli-hyper/Cargo.toml') && r.profile.test);\n          need(privateUnit.some(r => ['full', 'http1', 'http2', 'client', 'server', 'source-client-framing'].every(f => r.features.includes(f))), 'qualification private unit feature closure');\n"},{"at":7582,"insert_utf8":"          fs.writeFileSync(`${dir}/identity.json`, JSON.stringify({ partition: process.env.PARTITION, source_revision: process.env.GITHUB_SHA, target: process.env.TARGET_TRIPLE, runner_arch: process.env.RUNNER_ARCH, run_id: process.env.GITHUB_RUN_ID, run_attempt: process.env.GITHUB_RUN_ATTEMPT, evidence_scope: 'actual pre-gate compiler contexts and executable hashes; no immutable byte-archive or OS launch-image claim' }, null, 2) + '\\n');\n","remove_utf8":"          fs.writeFileSync(`${dir}/identity.json`, JSON.stringify({ partition: process.env.PARTITION, source_revision: process.env.GITHUB_SHA, target: process.env.TARGET_TRIPLE, runner_arch: process.env.RUNNER_ARCH, run_id: process.env.GITHUB_RUN_ID, run_attempt: process.env.GITHUB_RUN_ATTEMPT, evidence_scope: 'actual qualification-only full-private-Hyper workspace compiler contexts and executable hashes; not standalone default-shipping CLI evidence or immutable byte/launch archive' }, null, 2) + '\\n');\n"},{"at":8191,"insert_utf8":"        run: cargo clippy --locked --workspace --all-targets -- -D warnings\n","remove_utf8":"        run: |\n          set -euo pipefail\n          cargo clippy --locked --workspace --all-targets --exclude ssx-cli-hyper -- -D warnings\n          cargo clippy --locked -p ssx-cli-hyper --features full,source-client-framing --lib -- -D warnings\n"},{"at":9124,"insert_utf8":"          cargo test --locked --workspace\n","remove_utf8":"          cargo test --locked --workspace --features ssx-cli-hyper/full,ssx-cli-hyper/source-client-framing\n"},{"at":19872,"insert_utf8":"          cargo tree --locked --workspace -e features > target/rust-feature-graph.txt\n","remove_utf8":"          cargo tree --locked --workspace --exclude ssx-cli --exclude ssx-cli-check --exclude ssx-cli-openssl-store --exclude ssx-cli-hyper -e features > target/rust-feature-graph.txt\n"},{"at":20379,"insert_utf8":"","remove_utf8":"          cargo run --locked -p ssx-cli-check -- check T08A-TLS-ROLES --output target/rust-cli-tls-roles.json\n"},{"at":21597,"insert_utf8":"","remove_utf8":"\n  foundation-core:\n    name: Foundation core (${{ matrix.runner }})\n    runs-on: ${{ matrix.runner }}\n    timeout-minutes: 120\n    strategy:\n      fail-fast: false\n      matrix:\n        include:\n          - runner: ubuntu-24.04\n            target: x86_64-unknown-linux-gnu\n          - runner: ubuntu-24.04-arm\n            target: aarch64-unknown-linux-gnu\n    env:\n      CARGO_TERM_COLOR: always\n      RUST_BACKTRACE: 1\n    steps:\n      - uses: actions/checkout@v7\n        with:\n          fetch-depth: 0\n\n      - name: Confirm pinned toolchain\n        run: rustc --version --verbose\n\n      - name: Install mandatory offline replay isolation\n        run: |\n          sudo apt-get update\n          sudo apt-get install --yes bubblewrap apparmor\n          /usr/bin/bwrap --version\n          # Same namespace/capability flags as the native runner; no candidate runs here.\n          replay_setup_probe() {\n            /usr/bin/bwrap --unshare-user --unshare-net --unshare-pid --disable-userns \\\n              --cap-drop ALL --new-session --die-with-parent --ro-bind / / \\\n              --proc /proc --dev /dev -- /bin/sh -c \\\n              'cat /proc/self/attr/current; grep \"^CapEff:\" /proc/self/status; cat /proc/net/dev'\n          }\n          sudo cat /sys/kernel/security/apparmor/profiles | grep -E '^(bwrap|unpriv_bwrap)' || true\n          # Refuse an unexpected executable attachment instead of layering a second profile.\n          for policy in /etc/apparmor.d/*; do\n            if [ -f \"$policy\" ] && grep -Eq '^[[:space:]]*(profile[[:space:]]+[^[:space:]]+[[:space:]]+)?/usr/bin/bwrap([[:space:]]|$)' \"$policy\"; then\n              echo \"Existing bubblewrap policy: $policy\"\n              cat \"$policy\"\n              if ! grep -Eq '^[[:space:]]*profile[[:space:]]+bwrap[[:space:]]+/usr/bin/bwrap([[:space:]]|$)' \"$policy\"; then\n                echo \"Unexpected bwrap attachment identity; refusing policy replacement\" >&2\n                exit 1\n              fi\n            fi\n          done\n          if ! replay_setup_probe; then\n            sudo journalctl -k --no-pager -n 100 | grep -E 'apparmor|userns|bwrap' || true\n          fi\n          # Purpose-built bwrap setup profile plus capability-denied child stack.\n          # See docs/rust-rewrite/replay/README.md for upstream policy provenance.\n          sudo apparmor_parser --replace <<'PROFILE'\n          abi <abi/4.0>,\n          include <tunables/global>\n          profile bwrap /usr/bin/bwrap flags=(attach_disconnected,mediate_deleted) {\n            capability,\n            file rwlkm /{**,},\n            network,\n            unix,\n            ptrace,\n            signal,\n            userns,\n            mount,\n            umount,\n            pivot_root,\n            dbus,\n            pix /** -> &bwrap//&ssx_replay_unpriv,\n          }\n          profile ssx_replay_unpriv flags=(attach_disconnected,mediate_deleted) {\n            file rwlkm /{**,},\n            network,\n            unix,\n            ptrace,\n            signal,\n            userns,\n            mount,\n            umount,\n            pivot_root,\n            dbus,\n            pix /** -> &ssx_replay_unpriv,\n            audit deny capability,\n          }\n          PROFILE\n          replay_setup_probe\n\n      - name: Build pinned disposable native Valkey\n        timeout-minutes: 15\n        run: |\n          sh scripts/rust-valkey-build.sh \"$RUNNER_TEMP/ssx-valkey/bin\"\n          echo \"$RUNNER_TEMP/ssx-valkey/bin\" >> \"$GITHUB_PATH\"\n          valkey_server=\"$RUNNER_TEMP/ssx-valkey/bin/valkey-server\"\n          {\n            printf 'SLOPSEARX_TEST_VALKEY_SERVER=%s\\n' \"$valkey_server\"\n            printf 'SLOPSEARX_TEST_VALKEY_SERVER_SHA256=%s\\n' \"$(sha256sum \"$valkey_server\" | cut -d ' ' -f 1)\"\n          } >> \"$GITHUB_ENV\"\n\n      - name: Build actual-process HTTP fixture server\n        run: cargo build --locked -p ssx-http --example t02_fixture\n\n      - name: Locked workspace build\n        run: cargo build --locked --workspace --target \"${{ matrix.target }}\"\n\n      - name: Verify outbound TLS feature graph\n        run: |\n          cargo tree --locked --workspace --exclude ssx-cli --exclude ssx-cli-check --exclude ssx-cli-openssl-store --exclude ssx-cli-hyper -e features > target/rust-feature-graph.txt\n          grep -q 'rustls-platform-verifier' target/rust-feature-graph.txt\n          if grep -Eq '(^|[[:space:]])(native-tls|hyper-tls|openssl-sys)([[:space:]@v]|$)' target/rust-feature-graph.txt; then\n            echo \"unexpected native-TLS/OpenSSL backend in the locked feature graph\" >&2\n            exit 1\n          fi\n          cargo run --locked -p ssx-cli-check -- check T08A-TLS-ROLES --output target/rust-cli-tls-roles.json\n\n      - name: Fresh source-bound workspace compiler provenance\n        shell: bash\n        run: |\n          set -euo pipefail\n          umask 077\n          evidence=\"target/rust-evidence/partitions/core\"\n          mkdir -p \"$evidence\"\n          git ls-files -z | LC_ALL=C sort -z | xargs -0 sha256sum --zero > \"$evidence/source-before.sha256z\"\n          git ls-files --stage -z > \"$evidence/stage-before.z\"\n          [ \"$(git rev-parse HEAD)\" = \"$GITHUB_SHA\" ]\n          [ -z \"$(git status --porcelain)\" ]\n          rustc --version --verbose > \"$evidence/rustc.txt\"\n          cargo --version --verbose > \"$evidence/cargo.txt\"\n          sha256sum Cargo.toml Cargo.lock rust-toolchain.toml > \"$evidence/build-inputs.sha256\"\n          cargo build --locked --workspace --all-targets --features ssx-cli-hyper/full,ssx-cli-hyper/source-client-framing --message-format=json-render-diagnostics > \"$evidence/workspace.jsonl\"\n          PARTITION=core TARGET_TRIPLE=\"${{ matrix.target }}\" node <<'NODE'\n          const fs = require('node:fs'), path = require('node:path'), crypto = require('node:crypto');\n          const root = fs.realpathSync(process.cwd()), dir = `target/rust-evidence/partitions/${process.env.PARTITION}`;\n          const rows = fs.readFileSync(`${dir}/workspace.jsonl`, 'utf8').trim().split('\\n').map(JSON.parse);\n          const need = (v, s) => { if (!v) throw Error(s); };\n          need(rows.at(-1)?.reason === 'build-finished' && rows.at(-1).success === true, 'compiler completion');\n          const privateUnit = rows.filter(r => r.reason === 'compiler-artifact' && r.manifest_path === path.join(root, 'vendor/ssx-cli-hyper/Cargo.toml') && r.profile.test);\n          need(privateUnit.some(r => ['full', 'http1', 'http2', 'client', 'server', 'source-client-framing'].every(f => r.features.includes(f))), 'qualification private unit feature closure');\n          const selected = rows.filter(r => r.reason === 'compiler-artifact' && r.executable);\n          need(selected.length > 0, 'actual executable contexts');\n          const images = selected.map(artifact => {\n            const exe = path.resolve(artifact.executable), rel = path.relative(path.join(root, 'target'), exe);\n            need(rel && !rel.startsWith('..') && !path.isAbsolute(rel), 'compiler target confinement');\n            need(artifact.filenames.includes(artifact.executable), 'declared executable filename');\n            const info = fs.lstatSync(exe); need(info.isFile() && !info.isSymbolicLink(), 'regular compiler executable');\n            return { artifact, sha256: crypto.createHash('sha256').update(fs.readFileSync(exe)).digest('hex'), bytes: info.size, mode: info.mode & 0o777 };\n          });\n          fs.writeFileSync(`${dir}/compiler-images.json`, JSON.stringify(images, null, 2) + '\\n');\n          fs.writeFileSync(`${dir}/identity.json`, JSON.stringify({ partition: process.env.PARTITION, source_revision: process.env.GITHUB_SHA, target: process.env.TARGET_TRIPLE, runner_arch: process.env.RUNNER_ARCH, run_id: process.env.GITHUB_RUN_ID, run_attempt: process.env.GITHUB_RUN_ATTEMPT, evidence_scope: 'actual qualification-only full-private-Hyper workspace compiler contexts and executable hashes; not standalone default-shipping CLI evidence or immutable byte/launch archive' }, null, 2) + '\\n');\n          NODE\n\n      - name: Foundation lane evidence\n        env:\n          SLOPSEARX_TEST_TEMP_ROOT: ${{ runner.temp }}/t02-sockets-f03\n          TMPDIR: ${{ runner.temp }}/t02-sockets-f03\n          TMP: ${{ runner.temp }}/t02-sockets-f03\n          TEMP: ${{ runner.temp }}/t02-sockets-f03\n        run: |\n          mkdir \"$SLOPSEARX_TEST_TEMP_ROOT\"\n          trap 'rmdir \"$SLOPSEARX_TEST_TEMP_ROOT\" 2>/dev/null || true' EXIT\n          cargo xtask check F03 --output target/rust-evidence/F03.json\n\n      - name: Verify core source and index unchanged\n        if: always()\n        shell: bash\n        run: |\n          set -euo pipefail\n          evidence=\"target/rust-evidence/partitions/core\"\n          git ls-files -z | LC_ALL=C sort -z | xargs -0 sha256sum --zero > \"$evidence/source-after.sha256z\"\n          git ls-files --stage -z > \"$evidence/stage-after.z\"\n          cmp \"$evidence/source-before.sha256z\" \"$evidence/source-after.sha256z\"\n          cmp \"$evidence/stage-before.z\" \"$evidence/stage-after.z\"\n          [ \"$(git rev-parse HEAD)\" = \"$GITHUB_SHA\" ]\n          [ -z \"$(git status --porcelain)\" ]\n\n      - name: Retain core foundation partition evidence\n        if: always()\n        uses: actions/upload-artifact@v7\n        with:\n          name: rust-foundation-core-${{ matrix.target }}-${{ github.sha }}\n          path: |\n            target/rust-evidence/\n            target/rust-feature-graph.txt\n          if-no-files-found: warn\n"},{"at":35824,"insert_utf8":"          cargo tree --locked --workspace -e features > target/rust-feature-graph.txt\n","remove_utf8":"          cargo tree --locked --workspace --exclude ssx-cli --exclude ssx-cli-check --exclude ssx-cli-openssl-store --exclude ssx-cli-hyper -e features > target/rust-feature-graph.txt\n"},{"at":36331,"insert_utf8":"","remove_utf8":"          cargo run --locked -p ssx-cli-check -- check T08A-TLS-ROLES --output target/rust-cli-tls-roles.json\n"},{"at":37167,"insert_utf8":"          cargo build --locked --workspace --all-targets --message-format=json-render-diagnostics > \"$evidence/workspace.jsonl\"\n","remove_utf8":"          cargo build --locked --workspace --all-targets --features ssx-cli-hyper/full,ssx-cli-hyper/source-client-framing --message-format=json-render-diagnostics > \"$evidence/workspace.jsonl\"\n"},{"at":37948,"insert_utf8":"","remove_utf8":"          const privateUnit = rows.filter(r => r.reason === 'compiler-artifact' && r.manifest_path === path.join(root, 'vendor/ssx-cli-hyper/Cargo.toml') && r.profile.test);\n          need(privateUnit.some(r => ['full', 'http1', 'http2', 'client', 'server', 'source-client-framing'].every(f => r.features.includes(f))), 'qualification private unit feature closure');\n"},{"at":39236,"insert_utf8":"          fs.writeFileSync(`${dir}/identity.json`, JSON.stringify({ partition: process.env.PARTITION, source_revision: process.env.GITHUB_SHA, target: process.env.TARGET_TRIPLE, runner_arch: process.env.RUNNER_ARCH, run_id: process.env.GITHUB_RUN_ID, run_attempt: process.env.GITHUB_RUN_ATTEMPT, evidence_scope: 'actual pre-gate compiler contexts and executable hashes; no immutable byte-archive or OS launch-image claim' }, null, 2) + '\\n');\n","remove_utf8":"          fs.writeFileSync(`${dir}/identity.json`, JSON.stringify({ partition: process.env.PARTITION, source_revision: process.env.GITHUB_SHA, target: process.env.TARGET_TRIPLE, runner_arch: process.env.RUNNER_ARCH, run_id: process.env.GITHUB_RUN_ID, run_attempt: process.env.GITHUB_RUN_ATTEMPT, evidence_scope: 'actual qualification-only full-private-Hyper workspace compiler contexts and executable hashes; not standalone default-shipping CLI evidence or immutable byte/launch archive' }, null, 2) + '\\n');\n"},{"at":39760,"insert_utf8":"\n      - name: Foundation lane evidence\n        env:\n          SLOPSEARX_TEST_TEMP_ROOT: ${{ runner.temp }}/t02-sockets-f03\n          TMPDIR: ${{ runner.temp }}/t02-sockets-f03\n          TMP: ${{ runner.temp }}/t02-sockets-f03\n          TEMP: ${{ runner.temp }}/t02-sockets-f03\n        run: |\n          mkdir \"$SLOPSEARX_TEST_TEMP_ROOT\"\n          trap 'rmdir \"$SLOPSEARX_TEST_TEMP_ROOT\" 2>/dev/null || true' EXIT\n          cargo xtask check F03 --output target/rust-evidence/F03.json\n","remove_utf8":""},{"at":51943,"insert_utf8":"          cargo tree --locked --workspace -e features > target/rust-feature-graph.txt\n","remove_utf8":"          cargo tree --locked --workspace --exclude ssx-cli --exclude ssx-cli-check --exclude ssx-cli-openssl-store --exclude ssx-cli-hyper -e features > target/rust-feature-graph.txt\n"},{"at":52450,"insert_utf8":"","remove_utf8":"          cargo run --locked -p ssx-cli-check -- check T08A-TLS-ROLES --output target/rust-cli-tls-roles.json\n"},{"at":53287,"insert_utf8":"          cargo build --locked --workspace --all-targets --message-format=json-render-diagnostics > \"$evidence/workspace.jsonl\"\n","remove_utf8":"          cargo build --locked --workspace --all-targets --features ssx-cli-hyper/full,ssx-cli-hyper/source-client-framing --message-format=json-render-diagnostics > \"$evidence/workspace.jsonl\"\n"},{"at":54069,"insert_utf8":"","remove_utf8":"          const privateUnit = rows.filter(r => r.reason === 'compiler-artifact' && r.manifest_path === path.join(root, 'vendor/ssx-cli-hyper/Cargo.toml') && r.profile.test);\n          need(privateUnit.some(r => ['full', 'http1', 'http2', 'client', 'server', 'source-client-framing'].every(f => r.features.includes(f))), 'qualification private unit feature closure');\n"},{"at":55357,"insert_utf8":"          fs.writeFileSync(`${dir}/identity.json`, JSON.stringify({ partition: process.env.PARTITION, source_revision: process.env.GITHUB_SHA, target: process.env.TARGET_TRIPLE, runner_arch: process.env.RUNNER_ARCH, run_id: process.env.GITHUB_RUN_ID, run_attempt: process.env.GITHUB_RUN_ATTEMPT, evidence_scope: 'actual pre-gate compiler contexts and executable hashes; no immutable byte-archive or OS launch-image claim' }, null, 2) + '\\n');\n","remove_utf8":"          fs.writeFileSync(`${dir}/identity.json`, JSON.stringify({ partition: process.env.PARTITION, source_revision: process.env.GITHUB_SHA, target: process.env.TARGET_TRIPLE, runner_arch: process.env.RUNNER_ARCH, run_id: process.env.GITHUB_RUN_ID, run_attempt: process.env.GITHUB_RUN_ATTEMPT, evidence_scope: 'actual qualification-only full-private-Hyper workspace compiler contexts and executable hashes; not standalone default-shipping CLI evidence or immutable byte/launch archive' }, null, 2) + '\\n');\n"},{"at":59315,"insert_utf8":"    needs: [foundation-prerequisites, foundation-runtime, foundation-adapters]\n","remove_utf8":"    needs: [foundation-prerequisites, foundation-core, foundation-runtime, foundation-adapters]\n"},{"at":60196,"insert_utf8":"","remove_utf8":"      - name: Download core partition\n        id: download-core\n        if: always()\n        continue-on-error: true\n        uses: actions/download-artifact@v8\n        with:\n          name: rust-foundation-core-${{ matrix.target }}-${{ github.sha }}\n          path: ${{ runner.temp }}/foundation-partitions/core\n\n"},{"at":61355,"insert_utf8":"          for partition in prerequisites runtime adapters; do\n","remove_utf8":"          for partition in prerequisites core runtime adapters; do\n"},{"at":62149,"insert_utf8":"          printf 'source_revision=%s\\ntarget=%s\\nprerequisites=%s\\nruntime=%s\\nadapters=%s\\n' \"$GITHUB_SHA\" \"${{ matrix.target }}\" \"${{ needs.foundation-prerequisites.result }}\" \"${{ needs.foundation-runtime.result }}\" \"${{ needs.foundation-adapters.result }}\" > target/rust-evidence/foundation-partitions.txt\n","remove_utf8":"          printf 'source_revision=%s\\ntarget=%s\\nprerequisites=%s\\ncore=%s\\nruntime=%s\\nadapters=%s\\n' \"$GITHUB_SHA\" \"${{ matrix.target }}\" \"${{ needs.foundation-prerequisites.result }}\" \"${{ needs.foundation-core.result }}\" \"${{ needs.foundation-runtime.result }}\" \"${{ needs.foundation-adapters.result }}\" > target/rust-evidence/foundation-partitions.txt\n"},{"at":65145,"insert_utf8":"","remove_utf8":"          [ \"${{ needs.foundation-core.result }}\" = success ]\n"},{"at":65408,"insert_utf8":"","remove_utf8":"          [ \"${{ steps.download-core.outcome }}\" = success ]\n"},{"at":65900,"insert_utf8":"          for partition in prerequisites runtime adapters; do\n","remove_utf8":"          for partition in prerequisites core runtime adapters; do\n"},{"at":66433,"insert_utf8":"","remove_utf8":"          cmp target/rust-evidence/partitions/prerequisites/source-before.sha256z target/rust-evidence/partitions/core/source-before.sha256z\n"},{"at":66965,"insert_utf8":"          for (const partition of ['prerequisites', 'runtime', 'adapters']) {\n","remove_utf8":"          for (const partition of ['prerequisites', 'core', 'runtime', 'adapters']) {\n"}]"#######,
        )),
        ".pre-commit-config.yaml" => Ok((
            json!({"path":".pre-commit-config.yaml","bytes":13079,"sha256":"73cbb69a1089871ef718a8ebf4359897ebfc5c25edd9bcb8f8163308a875ba6e","git_blob":"30d86b2e3b6efc79a30fe973c1141b685b678b44","mode":"0644"}),
            json!({"path":".pre-commit-config.yaml","bytes":12910,"sha256":"ea4cc40b3aed49442d4cdc110420b29c997031101752e33280ea900780d4feb1","git_blob":"2e1002d93803ce0291dd4ea33a515ce75c9006b8","mode":"0644"}),
            2,
            "e3f6d4af5216c59c3773573e0e029421363286208c64c92d475555013db35739",
            r#######"[{"at":3165,"insert_utf8":"        exclude: ^(docs/experiments/evidence/(.*\\.(json|txt)|EXP-003/reproduce\\.md)|crates/xtask/fixtures/f13/native-record\\.json|docs/rust-rewrite/t06/validation\\.json)$|^(?:crates/ssx\\-domain/tests/fixtures/a05_previous_media_sanitizer\\.rs|crates/ssx\\-source\\-codecs/MAPPING\\-SOURCE\\-NOTICES\\.txt|crates/ssx\\-source\\-codecs/MAPPING\\-UPSTREAM\\-NOTICES\\.txt|crates/ssx\\-source\\-codecs/MAPPING\\-UPSTREAM\\-SINGLEBYTE\\-NOTICES\\.txt|crates/ssx\\-source\\-codecs/PYTHON\\-SOURCE\\-NOTICES\\.txt|crates/ssx\\-source\\-codecs/UNICODE\\-SOURCE\\-HEADERS\\.txt|crates/ssx\\-source\\-codecs/provenance/idna/RFC3454\\-ORIGINAL\\-NOTICE\\.txt|crates/ssx\\-source\\-codecs/provenance/idna/UNICODE\\-3\\.2\\-ORIGINAL\\-NOTICE\\.html|crates/ssx\\-source\\-codecs/provenance/idna/UnicodeCharacterDatabase\\-3\\.2\\.0\\.html|crates/ssx\\-source\\-codecs/provenance/idna/idna\\.py|crates/ssx\\-source\\-codecs/provenance/idna/inputs/CompositionExclusions\\-3\\.2\\.0\\.txt|crates/ssx\\-source\\-codecs/provenance/idna/inputs/SpecialCasing\\-16\\.0\\.0\\.txt|crates/ssx\\-source\\-codecs/provenance/idna/inputs/aliases\\.py|crates/ssx\\-source\\-codecs/provenance/idna/inputs/stringprep\\.py|crates/ssx\\-source\\-codecs/provenance/idna/mkstringprep\\.py|crates/ssx\\-source\\-codecs/provenance/idna/rfc3454\\.txt|crates/ssx\\-source\\-codecs/provenance/singlebyte\\-icu/koi8\\-t\\.ucm|docs/rust\\-rewrite/a05/libxml\\-safe\\-api\\-proposal\\.patch|docs/rust\\-rewrite/a05/surrogate\\-iconv\\-version\\-comparison\\.json|vendor/libxml/\\.cargo_vcs_info\\.json|vendor/libxml/\\.rustfmt\\.toml|vendor/libxml/CHANGELOG\\.md|vendor/libxml/README\\.md|vendor/libxml/appveyor\\.yml|vendor/libxml/src/wrapper\\.h|vendor/libxml/tests/VALGRIND\\.md|vendor/libxml/tests/resources/canonical_1_1/3_1_input\\.xml|vendor/libxml/tests/resources/canonical_1_1/3_1_output\\.xml|vendor/libxml/tests/resources/canonical_1_1/3_1_output_no_comment\\.xml|vendor/libxml/tests/resources/canonical_1_1/3_2_input\\.xml|vendor/libxml/tests/resources/canonical_1_1/Readme\\.md|vendor/libxml/tests/resources/canonical_exclusive/1_input\\.xml|vendor/libxml/tests/resources/canonical_exclusive/1_output\\.xml|vendor/libxml/tests/resources/example\\.html|vendor/libxml/tests/resources/file01_utf16le\\.xml|vendor/libxml/tests/resources/file01_utf16le_bom\\.xml|vendor/libxml/tests/resources/ids\\.xml|vendor/libxml/tests/resources/staff\\.xml|vendor/libxml/tests/resources/unformatted\\.xml|vendor/libxml2/a05\\-normal\\-data\\-nul\\.patch)$|^(?:contracts/unicode/python-text-source/Objects__unicodectype\\.c\\.txt|contracts/unicode/python-text-source/Objects__unicodetype_db\\.h\\.txt|contracts/unicode/python-text-source/Modules__unicodedata_db\\.h\\.txt)$|^docs/rust-rewrite/a05/source-amendments/3f1b9cca-cold-bootstrap\\.rs\\.frozen$|^docs/rust-rewrite/a07/qualification/pubchem-owner-clock-negative\\.stdout$|^logs/a05-codec-donor-final-alias-regression\\.log$|^(?:docs/rust\\-rewrite/t07/source\\-inputs/CPYTHON\\-unicodectype\\.c\\.txt|docs/rust\\-rewrite/t07/source\\-inputs/CPYTHON\\-unicodetype_db\\.h\\.txt|docs/rust\\-rewrite/t07/source\\-inputs/fastmcp\\-4\\.0\\.11\\.dist\\-info__LICENSE\\.txt|vendor/ssx\\-http\\-source/httparse/\\.cargo_vcs_info\\.json|vendor/ssx\\-http\\-source/httparse/LICENSE\\-MIT|vendor/ssx\\-http\\-source/hyper\\-util/\\.cargo_vcs_info\\.json|vendor/ssx\\-http\\-source/hyper/\\.cargo_vcs_info\\.json|vendor/ssx\\-http\\-source/reqwest/\\.cargo_vcs_info\\.json|vendor/ssx\\-http\\-source/reqwest/LICENSE\\-MIT|vendor/ssx\\-http\\-source/reqwest/src/source_pool/python\\-text\\-source/Modules__unicodedata_db\\.h\\.txt|vendor/ssx\\-http\\-source/reqwest/src/source_pool/python\\-text\\-source/Objects__unicodectype\\.c\\.txt|vendor/ssx\\-http\\-source/reqwest/src/source_pool/python\\-text\\-source/Objects__unicodetype_db\\.h\\.txt|vendor/ssx\\-sdk\\-auth/patches/rmcp\\-native\\-dev\\-fixtures\\.patch|vendor/ssx\\-sdk\\-auth/rmcp/\\.cargo_vcs_info\\.json|vendor/ssx\\-sdk\\-auth/rmcp/src/handler/server/resource\\.rs|vendor/ssx\\-sdk\\-auth/rmcp/tests/test_deserialization/tool_list_result\\.json|vendor/ssx\\-sdk\\-auth/rmcp/tests/test_message_schema/client_json_rpc_message_schema\\.json|vendor/ssx\\-sdk\\-auth/rmcp/tests/test_message_schema/client_json_rpc_message_schema_current\\.json|vendor/ssx\\-sdk\\-auth/rmcp/tests/test_message_schema/server_json_rpc_message_schema\\.json|vendor/ssx\\-sdk\\-auth/rmcp/tests/test_message_schema/server_json_rpc_message_schema_current\\.json|vendor/ssx\\-sdk\\-auth/rmcp/tests/test_tool_handler\\.rs|vendor/ssx\\-sdk\\-auth/rmcp/tests/test_with_js/\\.gitignore|vendor/ssx\\-sdk\\-auth/rmcp/tests/test_with_js/client\\.js|vendor/ssx\\-sdk\\-auth/rmcp/tests/test_with_js/server\\.js|vendor/ssx\\-sdk\\-auth/rmcp/tests/test_with_js/streamable_server\\.js)$|^(?:crates/ssx\\-source\\-codecs/provenance/idna2008/inputs/Objects__unicodetype_db\\.h\\.txt|crates/ssx\\-source\\-codecs/provenance/idna2008/inputs/unicodedata_db\\.h)$\n","remove_utf8":"        exclude: ^(docs/experiments/evidence/(.*\\.(json|txt)|EXP-003/reproduce\\.md)|crates/xtask/fixtures/f13/native-record\\.json|docs/rust-rewrite/t06/validation\\.json)$|^(?:crates/ssx\\-domain/tests/fixtures/a05_previous_media_sanitizer\\.rs|crates/ssx\\-source\\-codecs/MAPPING\\-SOURCE\\-NOTICES\\.txt|crates/ssx\\-source\\-codecs/MAPPING\\-UPSTREAM\\-NOTICES\\.txt|crates/ssx\\-source\\-codecs/MAPPING\\-UPSTREAM\\-SINGLEBYTE\\-NOTICES\\.txt|crates/ssx\\-source\\-codecs/PYTHON\\-SOURCE\\-NOTICES\\.txt|crates/ssx\\-source\\-codecs/UNICODE\\-SOURCE\\-HEADERS\\.txt|crates/ssx\\-source\\-codecs/provenance/idna/RFC3454\\-ORIGINAL\\-NOTICE\\.txt|crates/ssx\\-source\\-codecs/provenance/idna/UNICODE\\-3\\.2\\-ORIGINAL\\-NOTICE\\.html|crates/ssx\\-source\\-codecs/provenance/idna/UnicodeCharacterDatabase\\-3\\.2\\.0\\.html|crates/ssx\\-source\\-codecs/provenance/idna/idna\\.py|crates/ssx\\-source\\-codecs/provenance/idna/inputs/CompositionExclusions\\-3\\.2\\.0\\.txt|crates/ssx\\-source\\-codecs/provenance/idna/inputs/SpecialCasing\\-16\\.0\\.0\\.txt|crates/ssx\\-source\\-codecs/provenance/idna/inputs/aliases\\.py|crates/ssx\\-source\\-codecs/provenance/idna/inputs/stringprep\\.py|crates/ssx\\-source\\-codecs/provenance/idna/mkstringprep\\.py|crates/ssx\\-source\\-codecs/provenance/idna/rfc3454\\.txt|crates/ssx\\-source\\-codecs/provenance/singlebyte\\-icu/koi8\\-t\\.ucm|docs/rust\\-rewrite/a05/libxml\\-safe\\-api\\-proposal\\.patch|docs/rust\\-rewrite/a05/surrogate\\-iconv\\-version\\-comparison\\.json|vendor/libxml/\\.cargo_vcs_info\\.json|vendor/libxml/\\.rustfmt\\.toml|vendor/libxml/CHANGELOG\\.md|vendor/libxml/README\\.md|vendor/libxml/appveyor\\.yml|vendor/libxml/src/wrapper\\.h|vendor/libxml/tests/VALGRIND\\.md|vendor/libxml/tests/resources/canonical_1_1/3_1_input\\.xml|vendor/libxml/tests/resources/canonical_1_1/3_1_output\\.xml|vendor/libxml/tests/resources/canonical_1_1/3_1_output_no_comment\\.xml|vendor/libxml/tests/resources/canonical_1_1/3_2_input\\.xml|vendor/libxml/tests/resources/canonical_1_1/Readme\\.md|vendor/libxml/tests/resources/canonical_exclusive/1_input\\.xml|vendor/libxml/tests/resources/canonical_exclusive/1_output\\.xml|vendor/libxml/tests/resources/example\\.html|vendor/libxml/tests/resources/file01_utf16le\\.xml|vendor/libxml/tests/resources/file01_utf16le_bom\\.xml|vendor/libxml/tests/resources/ids\\.xml|vendor/libxml/tests/resources/staff\\.xml|vendor/libxml/tests/resources/unformatted\\.xml|vendor/libxml2/a05\\-normal\\-data\\-nul\\.patch)$|^(?:contracts/unicode/python-text-source/Objects__unicodectype\\.c\\.txt|contracts/unicode/python-text-source/Objects__unicodetype_db\\.h\\.txt|contracts/unicode/python-text-source/Modules__unicodedata_db\\.h\\.txt)$|^docs/rust-rewrite/a05/source-amendments/3f1b9cca-cold-bootstrap\\.rs\\.frozen$|^docs/rust-rewrite/a07/qualification/pubchem-owner-clock-negative\\.stdout$|^logs/a05-codec-donor-final-alias-regression\\.log$|^(?:docs/rust\\-rewrite/t07/source\\-inputs/CPYTHON\\-unicodectype\\.c\\.txt|docs/rust\\-rewrite/t07/source\\-inputs/CPYTHON\\-unicodetype_db\\.h\\.txt|docs/rust\\-rewrite/t07/source\\-inputs/fastmcp\\-4\\.0\\.11\\.dist\\-info__LICENSE\\.txt|vendor/ssx\\-http\\-source/httparse/\\.cargo_vcs_info\\.json|vendor/ssx\\-http\\-source/httparse/LICENSE\\-MIT|vendor/ssx\\-http\\-source/hyper\\-util/\\.cargo_vcs_info\\.json|vendor/ssx\\-http\\-source/hyper/\\.cargo_vcs_info\\.json|vendor/ssx\\-http\\-source/reqwest/\\.cargo_vcs_info\\.json|vendor/ssx\\-http\\-source/reqwest/LICENSE\\-MIT|vendor/ssx\\-http\\-source/reqwest/src/source_pool/python\\-text\\-source/Modules__unicodedata_db\\.h\\.txt|vendor/ssx\\-http\\-source/reqwest/src/source_pool/python\\-text\\-source/Objects__unicodectype\\.c\\.txt|vendor/ssx\\-http\\-source/reqwest/src/source_pool/python\\-text\\-source/Objects__unicodetype_db\\.h\\.txt|vendor/ssx\\-sdk\\-auth/patches/rmcp\\-native\\-dev\\-fixtures\\.patch|vendor/ssx\\-sdk\\-auth/rmcp/\\.cargo_vcs_info\\.json|vendor/ssx\\-sdk\\-auth/rmcp/src/handler/server/resource\\.rs|vendor/ssx\\-sdk\\-auth/rmcp/tests/test_deserialization/tool_list_result\\.json|vendor/ssx\\-sdk\\-auth/rmcp/tests/test_message_schema/client_json_rpc_message_schema\\.json|vendor/ssx\\-sdk\\-auth/rmcp/tests/test_message_schema/client_json_rpc_message_schema_current\\.json|vendor/ssx\\-sdk\\-auth/rmcp/tests/test_message_schema/server_json_rpc_message_schema\\.json|vendor/ssx\\-sdk\\-auth/rmcp/tests/test_message_schema/server_json_rpc_message_schema_current\\.json|vendor/ssx\\-sdk\\-auth/rmcp/tests/test_tool_handler\\.rs|vendor/ssx\\-sdk\\-auth/rmcp/tests/test_with_js/\\.gitignore|vendor/ssx\\-sdk\\-auth/rmcp/tests/test_with_js/client\\.js|vendor/ssx\\-sdk\\-auth/rmcp/tests/test_with_js/server\\.js|vendor/ssx\\-sdk\\-auth/rmcp/tests/test_with_js/streamable_server\\.js)$|^(?:crates/ssx\\-source\\-codecs/provenance/idna2008/inputs/Objects__unicodetype_db\\.h\\.txt|crates/ssx\\-source\\-codecs/provenance/idna2008/inputs/unicodedata_db\\.h)$|^(?:vendor/ssx\\-cli\\-hyper/\\.cargo_vcs_info\\.json)$|^vendor/ssx-cli-process-wrap/(?:\\.cargo_vcs_info\\.json|Cargo\\.toml\\.orig)$\n"},{"at":10600,"insert_utf8":"        exclude: ^docs/rust-rewrite/t06/(entities|validation)\\.json$|^docs/rust-rewrite/w06/(source\\.json|evidence/source-before-(composite-id-correction|unique-snapshot-ids)\\.json|evidence/review-public-source-outcomes\\.json)$|^docs/rust-rewrite/a10/(source-boundaries-v2\\.json|native-differences-v1\\.json)$|^docs/rust-rewrite/a11/(source-boundaries-v2|native-differences-v1)\\.json$|^(?:contracts/unicode/python-text-source/Modules__unicodedata_db\\.h\\.txt)$|^(?:crates/ssx\\-source\\-codecs/provenance/idna2008/inputs/unicodedata_db\\.h|crates/ssx\\-source\\-codecs/provenance/idna2008/inputs/unicodename_db\\.h|crates/ssx\\-source\\-codecs/src/idna2008_data\\.rs)$\n","remove_utf8":"        exclude: ^docs/rust-rewrite/t06/(entities|validation)\\.json$|^docs/rust-rewrite/w06/(source\\.json|evidence/source-before-(composite-id-correction|unique-snapshot-ids)\\.json|evidence/review-public-source-outcomes\\.json)$|^docs/rust-rewrite/a10/(source-boundaries-v2\\.json|native-differences-v1\\.json)$|^docs/rust-rewrite/a11/(source-boundaries-v2|native-differences-v1)\\.json$|^(?:contracts/unicode/python-text-source/Modules__unicodedata_db\\.h\\.txt)$|^(?:crates/ssx\\-source\\-codecs/provenance/idna2008/inputs/unicodedata_db\\.h|crates/ssx\\-source\\-codecs/provenance/idna2008/inputs/unicodename_db\\.h|crates/ssx\\-source\\-codecs/src/idna2008_data\\.rs)$|^(?:docs/rust\\-rewrite/t08a/cases\\.json)$\n"}]"#######,
        )),
        _ => Err("T07 stacked config unlisted path".into()),
    }
}
fn stacked_config_frame(
    expected: &Value,
    physical: &[u8],
    mode: &str,
    inverse: &Value,
) -> Result<Vec<u8>, String> {
    let (claim, parent, count, inverse_sha, _) = stacked_config_spec(text(expected, "path")?)?;
    need(
        *expected == claim,
        "T07 stacked config exact caller differs",
    )?;
    material(expected, physical, mode)?;
    need(
        inverse.as_array().is_some_and(|rows| rows.len() == count)
            && digest(&serde_json::to_vec(inverse).map_err(|e| e.to_string())?) == inverse_sha,
        "T07 stacked config closed inverse differs",
    )?;
    let prior = edits(physical, inverse)?;
    material(&parent, &prior, "0644")?;
    let mut forward = Vec::new();
    let (mut physical_cursor, mut prior_cursor) = (0usize, 0usize);
    for edit in inverse.as_array().ok_or("stacked config inverse rows")? {
        let at = edit["at"]
            .as_u64()
            .and_then(|n| usize::try_from(n).ok())
            .ok_or("stacked config inverse offset")?;
        let remove = text(edit, "remove_utf8")?;
        let insert = text(edit, "insert_utf8")?;
        let unchanged = at
            .checked_sub(physical_cursor)
            .ok_or("stacked config overlap")?;
        let prior_at = prior_cursor
            .checked_add(unchanged)
            .ok_or("stacked config prefix overflow")?;
        forward.push(json!({"at":prior_at,"remove_utf8":insert,"insert_utf8":remove}));
        physical_cursor = at
            .checked_add(remove.len())
            .ok_or("stacked config physical overflow")?;
        prior_cursor = prior_at
            .checked_add(insert.len())
            .ok_or("stacked config parent overflow")?;
    }
    need(
        edits(&prior, &json!(forward))? == physical,
        "T07 stacked config exact forward differs",
    )?;
    Ok(prior)
}
#[cfg(test)]
fn stacked_config_controls(root: &Path, path: &str, physical: &[u8]) -> Result<Vec<u8>, String> {
    let (claim, parent, _, _, raw_inverse) = stacked_config_spec(path)?;
    let inverse: Value = serde_json::from_str(raw_inverse).map_err(|e| e.to_string())?;
    let prior = stacked_config_frame(&claim, physical, "0644", &inverse)?;
    let git = std::process::Command::new("git")
        .args([
            "show",
            &format!("96574a69f9a287cf07ed11c222b6ae0313d5453b:{path}"),
        ])
        .current_dir(root)
        .output()
        .map_err(|e| e.to_string())?;
    let tree = std::process::Command::new("git")
        .args([
            "ls-tree",
            "96574a69f9a287cf07ed11c222b6ae0313d5453b",
            "--",
            path,
        ])
        .current_dir(root)
        .output()
        .map_err(|e| e.to_string())?;
    need(
        git.status.success()
            && git.stdout == prior
            && tree.status.success()
            && tree.stdout
                == format!("100644 blob {}\t{path}\n", text(&parent, "git_blob")?).as_bytes(),
        "T07 stacked config accepted965 Git bytes/mode/blob differ",
    )?;
    need(
        stacked_config_frame(&claim, &prior, "0644", &inverse).is_err(),
        "T07 stacked config old parent accepted as current",
    )?;
    need(
        stacked_config_frame(&claim, physical, "0600", &inverse).is_err(),
        "T07 stacked config changed current mode accepted",
    )?;
    let mut changed = physical.to_vec();
    changed.push(0);
    need(
        stacked_config_frame(&claim, &changed, "0644", &inverse).is_err(),
        "T07 stacked config changed current bytes accepted",
    )?;
    for key in ["path", "bytes", "sha256", "git_blob", "mode"] {
        let mut wrong = claim.clone();
        wrong[key] = json!("wrong");
        need(
            stacked_config_frame(&wrong, physical, "0644", &inverse).is_err(),
            "T07 stacked config wrong exact caller accepted",
        )?;
    }
    for index in 0..inverse
        .as_array()
        .ok_or("stacked config control rows")?
        .len()
    {
        for key in ["at", "remove_utf8", "insert_utf8"] {
            let mut wrong = inverse.clone();
            wrong[index][key] = json!("wrong");
            need(
                stacked_config_frame(&claim, physical, "0644", &wrong).is_err(),
                "T07 stacked config changed span accepted",
            )?;
        }
    }
    let mut missing = inverse.clone();
    missing
        .as_array_mut()
        .ok_or("stacked config missing rows")?
        .pop();
    let mut duplicate = inverse.clone();
    duplicate
        .as_array_mut()
        .ok_or("stacked config duplicate rows")?
        .push(inverse[0].clone());
    for wrong in [missing, duplicate] {
        need(
            stacked_config_frame(&claim, physical, "0644", &wrong).is_err(),
            "T07 stacked config missing/duplicate span accepted",
        )?;
    }
    Ok(prior)
}

// Exact additive #1080 audit and retained A06 producer placement. The original
// combined-source28-span proof below remains unchanged and sees only its own
// authenticated historical operand after this outer current-frame admission.
// Preregister the core lane fixture roots before any unchanged workspace test.
const CORE_FIXTURE_WORKFLOW_INVERSE: &str = r########"[{"at":21955,"insert_utf8":"","remove_utf8":"      - name: Configure private gateway fixture roots\n        env:\n          SLOPSEARX_INSTALL_TEST_ROOT: ${{ runner.temp }}/ssx-gateway-install\n          SLOPSEARX_T07_UNIT_TEST_ROOT: ${{ runner.temp }}/ssx-t07-unit-tests\n        run: |\n          printf 'SLOPSEARX_INSTALL_TEST_ROOT=%s\\n' \"$SLOPSEARX_INSTALL_TEST_ROOT\" >> \"$GITHUB_ENV\"\n          printf 'SLOPSEARX_T07_UNIT_TEST_ROOT=%s\\n' \"$SLOPSEARX_T07_UNIT_TEST_ROOT\" >> \"$GITHUB_ENV\"\n\n      - name: Prepare private gateway package fixtures\n        run: mkdir -m 700 \"$SLOPSEARX_INSTALL_TEST_ROOT\"\n\n"},{"at":31427,"insert_utf8":"","remove_utf8":"\n      - name: Retire gateway package fixtures\n        if: always()\n        run: rm -rf -- \"$SLOPSEARX_INSTALL_TEST_ROOT\"\n"}]"########;
fn core_fixture_workflow_claim() -> Value {
    json!({"path":".github/workflows/rust.yml","bytes":77327,"sha256":"a0f55fe4eaaed6039d8829fb3a003fbd5e6ce99dbf141c4e83760fe3d4e7a9bb","git_blob":"3975effbfa3df6200828737282ee990c02e17e84","mode":"0644"})
}
fn core_fixture_workflow_frame(
    expected: &Value,
    physical: &[u8],
    mode: &str,
    inverse: &Value,
) -> Result<Vec<u8>, String> {
    need(
        *expected == core_fixture_workflow_claim(),
        "T07 core fixture workflow exact caller differs",
    )?;
    material(expected, physical, mode)?;
    need(
        inverse.as_array().is_some_and(|x| x.len() == 2)
            && digest(&serde_json::to_vec(inverse).map_err(|e| e.to_string())?)
                == "f74353e38ad8a1ca06b1157b446bb0f8b6fe661c626b987309ee2f97afeb459a",
        "T07 core fixture workflow closed inverse differs",
    )?;
    let prior = edits(physical, inverse)?;
    material(&upstream_workflow_claim(), &prior, "0644")?;
    let mut forward = Vec::new();
    let (mut current_cursor, mut prior_cursor) = (0usize, 0usize);
    for row in inverse.as_array().ok_or("core fixture workflow spans")? {
        let at = row["at"]
            .as_u64()
            .and_then(|x| usize::try_from(x).ok())
            .ok_or("core fixture workflow offset")?;
        let remove = text(row, "remove_utf8")?;
        let insert = text(row, "insert_utf8")?;
        let prior_at = prior_cursor
            .checked_add(
                at.checked_sub(current_cursor)
                    .ok_or("core fixture workflow overlap")?,
            )
            .ok_or("core fixture workflow prefix overflow")?;
        forward.push(json!({"at":prior_at,"remove_utf8":insert,"insert_utf8":remove}));
        current_cursor = at
            .checked_add(remove.len())
            .ok_or("core fixture workflow current overflow")?;
        prior_cursor = prior_at
            .checked_add(insert.len())
            .ok_or("core fixture workflow prior overflow")?;
    }
    need(
        edits(&prior, &json!(forward))? == physical,
        "T07 core fixture workflow exact forward differs",
    )?;
    Ok(prior)
}
#[cfg(test)]
fn core_fixture_workflow_controls(root: &Path, physical: &[u8]) -> Result<Vec<u8>, String> {
    let claim = core_fixture_workflow_claim();
    let inverse: Value =
        serde_json::from_str(CORE_FIXTURE_WORKFLOW_INVERSE).map_err(|e| e.to_string())?;
    let prior = core_fixture_workflow_frame(&claim, physical, "0644", &inverse)?;
    let git = std::process::Command::new("git")
        .args([
            "show",
            "6eeeac802936ad6cd983f199b90e2991cfd0376d:.github/workflows/rust.yml",
        ])
        .current_dir(root)
        .output()
        .map_err(|e| e.to_string())?;
    let tree = std::process::Command::new("git")
        .args([
            "ls-tree",
            "6eeeac802936ad6cd983f199b90e2991cfd0376d",
            "--",
            ".github/workflows/rust.yml",
        ])
        .current_dir(root)
        .output()
        .map_err(|e| e.to_string())?;
    need(git.status.success() && git.stdout==prior && tree.status.success() && tree.stdout==b"100644 blob 49dc0346f5c72f4d8f806006782bb0e2d2ec6682\t.github/workflows/rust.yml\n","T07 core fixture workflow prior Git bytes/mode/blob")?;
    need(
        core_fixture_workflow_frame(&claim, &prior, "0644", &inverse).is_err(),
        "T07 core fixture workflow old-as-current accepted",
    )?;
    need(
        core_fixture_workflow_frame(&claim, physical, "0600", &inverse).is_err(),
        "T07 core fixture workflow wrong mode accepted",
    )?;
    let mut changed = physical.to_vec();
    changed.push(0);
    need(
        core_fixture_workflow_frame(&claim, &changed, "0644", &inverse).is_err(),
        "T07 core fixture workflow changed current accepted",
    )?;
    for key in ["path", "bytes", "sha256", "git_blob", "mode"] {
        let mut wrong = claim.clone();
        wrong[key] = json!("wrong");
        need(
            core_fixture_workflow_frame(&wrong, physical, "0644", &inverse).is_err(),
            "T07 core fixture workflow wrong caller accepted",
        )?;
    }
    for index in 0..2 {
        for key in ["at", "remove_utf8", "insert_utf8"] {
            let mut wrong = inverse.clone();
            wrong[index][key] = json!("wrong");
            need(
                core_fixture_workflow_frame(&claim, physical, "0644", &wrong).is_err(),
                "T07 core fixture workflow modified span accepted",
            )?;
        }
    }
    for which in 0..3 {
        let mut wrong = inverse.clone();
        let rows = wrong.as_array_mut().ok_or("core fixture workflow rows")?;
        if which == 0 {
            rows.remove(0);
        } else if which == 1 {
            rows.push(rows[0].clone());
        } else {
            rows.swap(0, 1);
        }
        need(
            core_fixture_workflow_frame(&claim, physical, "0644", &wrong).is_err(),
            "T07 core fixture workflow missing/duplicate/moved spans accepted",
        )?;
    }
    Ok(prior)
}
const UPSTREAM_WORKFLOW_INVERSE: &str = r########"[{"at":19545,"insert_utf8":"      - name: Native scholarly adapters and Expat boundary qualification\n        run: cargo xtask check A06 --output target/rust-evidence/A06.json\n\n","remove_utf8":""},{"at":56385,"insert_utf8":"","remove_utf8":"      - name: Native scholarly adapters and Expat boundary qualification\n        env:\n          CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUNNER: /usr/bin/env -u SSL_CERT_FILE -u SSL_CERT_DIR\n          CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_RUNNER: /usr/bin/env -u SSL_CERT_FILE -u SSL_CERT_DIR\n        run: cargo xtask check A06 --output target/rust-evidence/A06.json\n\n"},{"at":68446,"insert_utf8":"","remove_utf8":"\n      - name: Build native upstream advisory view and capture actual metadata\n        shell: bash\n        run: |\n          set -euo pipefail\n          umask 077\n          evidence=target/rust-evidence/upstream-advisories\n          mkdir -p \"$evidence\"\n          test \"$(git rev-parse HEAD)\" = \"$GITHUB_SHA\"\n          test -z \"$(git status --porcelain)\"\n          cargo deny --version > \"$evidence/scanner-version.txt\"\n          cargo --version --verbose > \"$evidence/cargo-version.txt\"\n          rustc --version --verbose > \"$evidence/rustc-version.txt\"\n          cargo build --locked -p xtask --example upstream_advisory_view --message-format=json-render-diagnostics > \"$evidence/helper-build.jsonl\"\n          cargo metadata --locked --format-version 1 > \"$evidence/raw-metadata.json\" 2> \"$evidence/metadata.stderr\"\n          target/debug/examples/upstream_advisory_view derive \"$evidence/raw-metadata.json\" \"$evidence/view\"\n      - name: Prove private fork advisory coverage with the pinned scanner\n        shell: bash\n        run: |\n          set -euo pipefail\n          umask 077\n          evidence=target/rust-evidence/upstream-advisories\n          fixture=\"$evidence/fixtures\"\n          target/debug/examples/upstream_advisory_view fixtures \"$evidence/raw-metadata.json\" \"$fixture\"\n          cargo deny --version > \"$fixture/cargo-deny-version.stdout\"\n          while IFS=$'\\t' read -r name source config; do\n            source=\"$fixture/$source\"\n            config=\"$fixture/$config\"\n            test -n \"$name\" && test -d \"$source\" && test -f \"$config\"\n            git -C \"$source\" init --initial-branch=main\n            git -C \"$source\" add --all\n            git -C \"$source\" -c user.name='SlopSearX scanner fixture' -c user.email='scanner-fixture@example.invalid' commit -s -m 'Synthetic local advisory fixture'\n            cargo deny --config \"$config\" fetch db > \"$fixture/$name-fetch.stdout\" 2> \"$fixture/$name-fetch.stderr\"\n          done < \"$fixture/fixture-dbs.tsv\"\n          while IFS=$'\\t' read -r name config metadata; do\n            config=\"$fixture/$config\"\n            metadata=\"$fixture/$metadata\"\n            test -n \"$name\" && test -f \"$config\" && test -f \"$metadata\"\n            if cargo deny --format json --locked --offline --workspace --metadata-path \"$metadata\" --config \"$config\" check advisories > \"$fixture/$name.stdout\" 2> \"$fixture/$name.stderr\"; then\n              code=0\n            else\n              code=$?\n            fi\n            printf '%s\\n' \"$code\" > \"$fixture/$name.exit\"\n          done < \"$fixture/fixture-runs.tsv\"\n          target/debug/examples/upstream_advisory_view verify-fixtures \"$fixture\"\n      - name: Fetch fresh RustSec data and check upstream fork advisories\n        shell: bash\n        run: |\n          set -euo pipefail\n          umask 077\n          evidence=target/rust-evidence/upstream-advisories\n          if cargo deny --locked --workspace fetch db > \"$evidence/db-fetch.stdout\" 2> \"$evidence/db-fetch.stderr\"; then\n            SSX_AUDIT_FETCH_EXIT=0\n          else\n            SSX_AUDIT_FETCH_EXIT=$?\n          fi\n          SSX_AUDIT_SCAN_EXIT=not-run\n          if [ \"$SSX_AUDIT_FETCH_EXIT\" -eq 0 ]; then\n            if cargo deny --format json --locked --offline --workspace --metadata-path \"$evidence/view/upstream-advisory-metadata.json\" check advisories > \"$evidence/scan.stdout\" 2> \"$evidence/scan.stderr\"; then\n              SSX_AUDIT_SCAN_EXIT=0\n            else\n              SSX_AUDIT_SCAN_EXIT=$?\n            fi\n          fi\n          export SSX_AUDIT_FETCH_EXIT SSX_AUDIT_SCAN_EXIT\n          node <<'NODE'\n          const fs = require('node:fs'), path = require('node:path'), os = require('node:os');\n          const crypto = require('node:crypto'), cp = require('node:child_process');\n          const evidence = 'target/rust-evidence/upstream-advisories';\n          const need = (ok, why) => { if (!ok) throw Error(why); };\n          const hashFile = file => {\n            const info = fs.lstatSync(file);\n            need(info.isFile() && !info.isSymbolicLink() && info.size <= 32 * 1024 * 1024, `bounded regular evidence: ${file}`);\n            return { path: file, bytes: info.size, sha256: crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex') };\n          };\n          const git = args => {\n            const value = cp.spawnSync('git', args, { encoding: 'utf8', timeout: 10000, maxBuffer: 65536 });\n            need(value.status === 0, `git evidence failed: ${args.join(' ')}`);\n            return value.stdout.trim();\n          };\n          const version = fs.readFileSync(`${evidence}/scanner-version.txt`, 'utf8').trim();\n          need(version === 'cargo-deny 0.20.2', 'pinned scanner version');\n          const source = git(['rev-parse', 'HEAD']);\n          need(source === process.env.GITHUB_SHA && git(['status', '--porcelain']) === '', 'source revision or clean tree changed');\n          const fetchExit = Number(process.env.SSX_AUDIT_FETCH_EXIT);\n          const scanExit = process.env.SSX_AUDIT_SCAN_EXIT === 'not-run' ? null : Number(process.env.SSX_AUDIT_SCAN_EXIT);\n          const databases = [];\n          if (fetchExit === 0) {\n            const dbRoot = path.join(process.env.CARGO_HOME || path.join(os.homedir(), '.cargo'), 'advisory-dbs');\n            const entries = fs.readdirSync(dbRoot, { withFileTypes: true });\n            need(entries.length <= 32, 'advisory database directory bound');\n            for (const entry of entries) {\n              if (!entry.isDirectory() || entry.isSymbolicLink()) continue;\n              const dir = path.join(dbRoot, entry.name);\n              const origin = git(['-C', dir, 'remote', 'get-url', 'origin']);\n              need(origin.replace(/\\/?$/, '').replace(/\\.git$/, '').toLowerCase() === 'https://github.com/rustsec/advisory-db', 'unexpected production advisory database origin');\n              databases.push({ path: dir, origin, commit: git(['-C', dir, 'rev-parse', 'HEAD']), tree: git(['-C', dir, 'rev-parse', 'HEAD^{tree}']) });\n            }\n            need(databases.length === 1, 'one freshly fetched production RustSec database required');\n          }\n          const files = [\n            'Cargo.toml', 'Cargo.lock', 'deny.toml',\n            'crates/xtask/examples/upstream_advisory_view.rs',\n            'docs/rust-rewrite/ci/upstream-fork-advisories.json',\n            `${evidence}/raw-metadata.json`, `${evidence}/view/upstream-advisory-metadata.json`,\n            `${evidence}/view/identity-receipt.json`, `${evidence}/db-fetch.stdout`, `${evidence}/db-fetch.stderr`,\n            ...[`${evidence}/scan.stdout`, `${evidence}/scan.stderr`].filter(file => fs.existsSync(file))\n          ].map(hashFile);\n          const receipt = {\n            schema: 1, scope: 'Inherited upstream Rust crate advisories for reviewed current path forks; not physical resolution, private-patch safety, native C advisory coverage, or shipping/TLS evidence',\n            source_revision: source, run_id: process.env.GITHUB_RUN_ID, run_attempt: process.env.GITHUB_RUN_ATTEMPT,\n            recorded_utc: new Date().toISOString(), scanner_version: version,\n            metadata_producer: { argv: ['cargo', 'metadata', '--locked', '--format-version', '1'], exit: 0 },\n            database_fetch: { argv: ['cargo', 'deny', '--locked', '--workspace', 'fetch', 'db'], exit: fetchExit },\n            scan: { argv: ['cargo', 'deny', '--format', 'json', '--locked', '--offline', '--workspace', '--metadata-path', `${evidence}/view/upstream-advisory-metadata.json`, 'check', 'advisories'], exit: scanExit, skipped_reason: scanExit === null ? 'fresh database fetch failed' : null },\n            databases, files\n          };\n          fs.writeFileSync(`${evidence}/fresh-scan-receipt.json`, JSON.stringify(receipt, null, 2) + '\\n');\n          NODE\n          test \"$SSX_AUDIT_FETCH_EXIT\" -eq 0\n          test \"$SSX_AUDIT_SCAN_EXIT\" = 0\n      - name: Retain upstream fork advisory coverage evidence\n        if: always()\n        uses: actions/upload-artifact@v7\n        with:\n          name: upstream-fork-advisories-${{ github.sha }}\n          path: target/rust-evidence/upstream-advisories/\n          if-no-files-found: error\n          retention-days: 14\n"}]"########;
fn upstream_workflow_claim() -> Value {
    json!({"path":".github/workflows/rust.yml","bytes":76651,"sha256":"b4836eaab7d6ffcdb582fa25d5cf76b4ccc7f4f6126d19ca21ab64402a9c78e4","git_blob":"49dc0346f5c72f4d8f806006782bb0e2d2ec6682","mode":"0644"})
}
fn upstream_workflow_frame(
    expected: &Value,
    physical: &[u8],
    mode: &str,
    inverse: &Value,
) -> Result<Vec<u8>, String> {
    need(
        *expected == upstream_workflow_claim(),
        "T07 advisory workflow exact caller differs",
    )?;
    material(expected, physical, mode)?;
    need(
        inverse.as_array().is_some_and(|x| x.len() == 3)
            && digest(&serde_json::to_vec(inverse).map_err(|e| e.to_string())?)
                == "64fb95b7bf0f7785eadf439e05152f054c7fe10064481240c2606ec2414887d3",
        "T07 advisory workflow closed inverse differs",
    )?;
    let prior = edits(physical, inverse)?;
    let (baseline, _, _, _, _) = stacked_config_spec(".github/workflows/rust.yml")?;
    material(&baseline, &prior, "0644")?;
    let mut forward = Vec::new();
    let (mut current_cursor, mut prior_cursor) = (0usize, 0usize);
    for row in inverse.as_array().ok_or("advisory workflow spans")? {
        let at = row["at"]
            .as_u64()
            .and_then(|x| usize::try_from(x).ok())
            .ok_or("advisory workflow offset")?;
        let remove = text(row, "remove_utf8")?;
        let insert = text(row, "insert_utf8")?;
        let prior_at = prior_cursor
            .checked_add(
                at.checked_sub(current_cursor)
                    .ok_or("advisory workflow overlap")?,
            )
            .ok_or("advisory workflow prefix overflow")?;
        forward.push(json!({"at":prior_at,"remove_utf8":insert,"insert_utf8":remove}));
        current_cursor = at
            .checked_add(remove.len())
            .ok_or("advisory workflow current overflow")?;
        prior_cursor = prior_at
            .checked_add(insert.len())
            .ok_or("advisory workflow prior overflow")?;
    }
    need(
        edits(&prior, &json!(forward))? == physical,
        "T07 advisory workflow exact forward differs",
    )?;
    Ok(prior)
}
#[cfg(test)]
fn upstream_workflow_controls(root: &Path, physical: &[u8]) -> Result<Vec<u8>, String> {
    let claim = upstream_workflow_claim();
    let inverse: Value =
        serde_json::from_str(UPSTREAM_WORKFLOW_INVERSE).map_err(|e| e.to_string())?;
    let prior = upstream_workflow_frame(&claim, physical, "0644", &inverse)?;
    let git = std::process::Command::new("git")
        .args([
            "show",
            "8dc52e72bb2e753eef75c578ab48cfe97ecb1b01:.github/workflows/rust.yml",
        ])
        .current_dir(root)
        .output()
        .map_err(|e| e.to_string())?;
    let tree = std::process::Command::new("git")
        .args([
            "ls-tree",
            "8dc52e72bb2e753eef75c578ab48cfe97ecb1b01",
            "--",
            ".github/workflows/rust.yml",
        ])
        .current_dir(root)
        .output()
        .map_err(|e| e.to_string())?;
    need(git.status.success() && git.stdout==prior && tree.status.success() && tree.stdout==b"100644 blob 71743ac9ebd564b71c46fa75f4ea8816de8d8093\t.github/workflows/rust.yml\n","T07 advisory workflow prior Git bytes/mode/blob")?;
    need(
        upstream_workflow_frame(&claim, &prior, "0644", &inverse).is_err(),
        "T07 advisory workflow old-as-current accepted",
    )?;
    need(
        upstream_workflow_frame(&claim, physical, "0600", &inverse).is_err(),
        "T07 advisory workflow wrong mode accepted",
    )?;
    let mut changed = physical.to_vec();
    changed.push(0);
    need(
        upstream_workflow_frame(&claim, &changed, "0644", &inverse).is_err(),
        "T07 advisory workflow changed current accepted",
    )?;
    for key in ["path", "bytes", "sha256", "git_blob", "mode"] {
        let mut wrong = claim.clone();
        wrong[key] = json!("wrong");
        need(
            upstream_workflow_frame(&wrong, physical, "0644", &inverse).is_err(),
            "T07 advisory workflow wrong caller accepted",
        )?;
    }
    for index in 0..3 {
        for key in ["at", "remove_utf8", "insert_utf8"] {
            let mut wrong = inverse.clone();
            wrong[index][key] = json!("wrong");
            need(
                upstream_workflow_frame(&claim, physical, "0644", &wrong).is_err(),
                "T07 advisory workflow modified span accepted",
            )?;
        }
    }
    for which in 0..3 {
        let mut wrong = inverse.clone();
        let rows = wrong.as_array_mut().ok_or("advisory workflow rows")?;
        if which == 0 {
            rows.remove(0);
        } else if which == 1 {
            rows.push(rows[0].clone());
        } else {
            rows.swap(0, 1);
        }
        need(
            upstream_workflow_frame(&claim, physical, "0644", &wrong).is_err(),
            "T07 advisory workflow missing/duplicate/moved spans accepted",
        )?;
    }
    Ok(prior)
}
fn config_frame(
    expected: &Value,
    physical: &[u8],
    mode: &str,
    inverse: &Value,
) -> Result<Vec<u8>, String> {
    let (claim, bytes, sha, inverse_sha, _) = config_spec(text(expected, "path")?)?;
    let (current_claim, _, _, _, raw_current_inverse) =
        stacked_config_spec(text(expected, "path")?)?;
    let current_inverse: Value =
        serde_json::from_str(raw_current_inverse).map_err(|e| e.to_string())?;
    let admitted = stacked_config_frame(&current_claim, physical, mode, &current_inverse)?;
    let physical = admitted.as_slice();
    let accepted = if expected["path"] == ".github/workflows/rust.yml" {
        let env_inverse: Value =
            serde_json::from_str(WORKFLOW_ENV_INVERSE).map_err(|e| e.to_string())?;
        let smoke_inverse: Value =
            serde_json::from_str(WORKFLOW_SMOKE_INVERSE).map_err(|e| e.to_string())?;
        let main_inverse: Value =
            serde_json::from_str(WORKFLOW_MAIN_JOIN_INVERSE).map_err(|e| e.to_string())?;
        let smoke_physical =
            workflow_main_join_frame(&workflow_main_join_claim(), physical, mode, &main_inverse)?;
        let accepted_current = workflow_smoke_frame(
            &workflow_smoke_claim(),
            &smoke_physical,
            mode,
            &smoke_inverse,
        )?;
        workflow_env_frame(&workflow_env_claim(), &accepted_current, mode, &env_inverse)?
    } else {
        physical.to_vec()
    };
    let physical = accepted.as_slice();
    need(
        *expected == claim && mode == "0644" && physical.len() == bytes && digest(physical) == sha,
        "T07 unit-route configuration current/caller differs",
    )?;
    need(
        digest(&serde_json::to_vec(inverse).map_err(|e| e.to_string())?) == inverse_sha,
        "T07 unit-route configuration ordered inverse differs",
    )?;
    let prior = edits(physical, inverse)?;
    material(expected, &prior, "0644")?;
    Ok(prior)
}
fn current_config(root: &Path, expected: &Value) -> Result<Vec<u8>, String> {
    let path = text(expected, "path")?;
    let (_, _, _, _, raw_inverse) = config_spec(path)?;
    let source = root.join(path);
    let meta = std::fs::symlink_metadata(&source).map_err(|e| e.to_string())?;
    need(
        meta.is_file() && !meta.file_type().is_symlink(),
        "T07 unit-route config must be regular",
    )?;
    let physical = std::fs::read(&source).map_err(|e| e.to_string())?;
    let inverse: Value = serde_json::from_str(raw_inverse).map_err(|e| e.to_string())?;
    let mode = permissions(&source)?;
    let admitted = if path == ".github/workflows/rust.yml" {
        let core: Value =
            serde_json::from_str(CORE_FIXTURE_WORKFLOW_INVERSE).map_err(|e| e.to_string())?;
        let physical =
            core_fixture_workflow_frame(&core_fixture_workflow_claim(), &physical, &mode, &core)?;
        let outer: Value =
            serde_json::from_str(UPSTREAM_WORKFLOW_INVERSE).map_err(|e| e.to_string())?;
        upstream_workflow_frame(&upstream_workflow_claim(), &physical, &mode, &outer)?
    } else {
        physical
    };
    let prior = config_frame(expected, &admitted, &mode, &inverse)?;
    let git = std::process::Command::new("git")
        .args(["cat-file", "blob", text(expected, "git_blob")?])
        .current_dir(root)
        .output()
        .map_err(|e| e.to_string())?;
    need(
        git.status.success() && git.stdout == prior,
        "T07 unit-route configuration Git frame differs",
    )?;
    Ok(prior)
}
pub(super) fn actual_bytes(
    root: &Path,
    expected: &Value,
    before: &[u8],
) -> Result<Vec<u8>, String> {
    let doc = load(root)?;
    let selected = row(&doc, expected)?;
    let actual = if selected["current"]["path"] == "crates/xtask/src/main.rs" {
        current_main(root, &selected["current"])?
    } else {
        current_config(root, &selected["current"])?
    };
    validate(
        selected,
        before,
        &actual,
        &permissions(&root.join(text(expected, "path")?))?,
    )?;
    for row in doc["transitions"].as_array().ok_or("T07 unit-route rows")? {
        let before = read(root, &row["archive"])?;
        let actual = if row["current"]["path"] == "crates/xtask/src/main.rs" {
            current_main(root, &row["current"])?
        } else {
            current_config(root, &row["current"])?
        };
        validate(
            row,
            &before,
            &actual,
            &permissions(&root.join(text(&row["current"], "path")?))?,
        )?;
    }
    Ok(actual)
}
#[cfg(test)]
pub(super) fn controls(root: &Path) -> Result<(), String> {
    let doc = load(root)?;
    for selected in doc["transitions"].as_array().ok_or("T07 unit-route rows")? {
        let expected = &selected["prior"];
        let before = read(root, &selected["archive"])?;
        let actual = actual_bytes(root, expected, &before)?;
        if selected["current"]["path"] != "crates/xtask/src/main.rs" {
            let claim = &selected["current"];
            let path = text(claim, "path")?;
            let physical = std::fs::read(root.join(path)).map_err(|e| e.to_string())?;
            let physical = if path == ".github/workflows/rust.yml" {
                let physical = core_fixture_workflow_controls(root, &physical)?;
                upstream_workflow_controls(root, &physical)?
            } else {
                physical
            };
            let admitted = stacked_config_controls(root, path, &physical)?;
            if path == ".github/workflows/rust.yml" {
                let physical = workflow_main_join_controls(&admitted)?;
                let physical = workflow_smoke_controls(&physical)?;
                let current = workflow_env_claim();
                let env_inverse: Value =
                    serde_json::from_str(WORKFLOW_ENV_INVERSE).map_err(|e| e.to_string())?;
                let concurrency_inverse: Value = serde_json::from_str(WORKFLOW_CONCURRENCY_INVERSE)
                    .map_err(|e| e.to_string())?;
                let concurrency_prior =
                    workflow_concurrency_frame(&current, &physical, "0644", &concurrency_inverse)?;
                need(
                    workflow_concurrency_frame(
                        &current,
                        &concurrency_prior,
                        "0644",
                        &concurrency_inverse,
                    )
                    .is_err(),
                    "T07 workflow concurrency historical current accepted",
                )?;
                for key in ["at", "remove_utf8", "insert_utf8"] {
                    let mut wrong = concurrency_inverse.clone();
                    wrong[0][key] = if key == "at" {
                        json!(1)
                    } else {
                        json!("unlisted")
                    };
                    need(
                        workflow_concurrency_frame(&current, &physical, "0644", &wrong).is_err(),
                        "T07 workflow concurrency altered span accepted",
                    )?;
                }
                let mut missing = concurrency_inverse.clone();
                missing.as_array_mut().unwrap().pop();
                let mut extra = concurrency_inverse.clone();
                extra
                    .as_array_mut()
                    .unwrap()
                    .push(concurrency_inverse[0].clone());
                for wrong in [missing, extra] {
                    need(
                        workflow_concurrency_frame(&current, &physical, "0644", &wrong).is_err(),
                        "T07 workflow concurrency missing/duplicate/extra span accepted",
                    )?;
                }
                let profile_inverse: Value =
                    serde_json::from_str(WORKFLOW_PROFILE_INVERSE).map_err(|e| e.to_string())?;
                need(
                    edits(&concurrency_prior, &profile_inverse)?
                        == std::process::Command::new("git")
                            .args([
                                "cat-file",
                                "blob",
                                "8fcc18490bc846f66ef4a6aff3989760cb20a980",
                            ])
                            .current_dir(root)
                            .output()
                            .map_err(|e| e.to_string())?
                            .stdout,
                    "T07 runtime profile exact accepted workflow Git bytes differ",
                )?;
                for selected in 0..2 {
                    let mut wrong = profile_inverse.clone();
                    wrong[selected]["at"] = json!(1);
                    need(
                        edits(&concurrency_prior, &wrong).is_err(),
                        "T07 runtime profile moved span accepted",
                    )?;
                }
                let parent = workflow_env_frame(&current, &physical, "0644", &env_inverse)?;
                let (_, bytes, sha, _, _) = config_spec(path)?;
                need(
                    parent.len() == bytes && digest(&parent) == sha,
                    "T07 workflow env inverse positive differs",
                )?;
                // The old valid historical operand is no longer a current workflow.
                need(
                    workflow_env_frame(&current, &parent, "0644", &env_inverse).is_err(),
                    "T07 workflow env stale historical current accepted",
                )?;
                for key in ["path", "sha256", "bytes", "git_blob", "mode"] {
                    let mut wrong = current.clone();
                    wrong[key] = json!("unlisted");
                    need(
                        workflow_env_frame(&wrong, &physical, "0644", &env_inverse).is_err(),
                        "T07 workflow env wrong current identity accepted",
                    )?;
                }
                need(
                    workflow_env_frame(&current, &physical, "0600", &env_inverse).is_err(),
                    "T07 workflow env wrong current mode accepted",
                )?;
                let mut changed = physical.clone();
                changed[0] ^= 1;
                need(
                    workflow_env_frame(&current, &changed, "0644", &env_inverse).is_err(),
                    "T07 workflow env wrong current digest accepted",
                )?;
                for key in ["at", "remove_utf8", "insert_utf8"] {
                    let mut wrong = env_inverse.clone();
                    wrong[0][key] = if key == "at" {
                        json!(1)
                    } else {
                        json!("unlisted")
                    };
                    need(
                        workflow_env_frame(&current, &physical, "0644", &wrong).is_err(),
                        "T07 workflow env altered span accepted",
                    )?;
                }
                let mut missing = env_inverse.clone();
                missing.as_array_mut().unwrap().pop();
                let mut extra = env_inverse.clone();
                extra.as_array_mut().unwrap().push(env_inverse[0].clone());
                for wrong in [missing, extra] {
                    need(
                        workflow_env_frame(&current, &physical, "0644", &wrong).is_err(),
                        "T07 workflow env missing/duplicate/extra span accepted",
                    )?;
                }
            }
            let (_, _, _, _, raw) = config_spec(path)?;
            let inverse: Value = serde_json::from_str(raw).map_err(|e| e.to_string())?;
            need(
                config_frame(claim, &physical, "0644", &inverse)? == actual,
                "T07 config inverse positive differs",
            )?;
            for key in ["path", "sha256", "bytes", "git_blob", "mode"] {
                let mut wrong = claim.clone();
                wrong[key] = json!("unlisted");
                need(
                    config_frame(&wrong, &physical, "0644", &inverse).is_err(),
                    "T07 config wrong caller accepted",
                )?;
            }
            need(
                config_frame(claim, &physical, "0600", &inverse).is_err(),
                "T07 config wrong mode accepted",
            )?;
            let mut bad = physical.clone();
            bad.push(0);
            need(
                config_frame(claim, &bad, "0644", &inverse).is_err(),
                "T07 config altered current accepted",
            )?;
            for key in ["at", "remove_utf8", "insert_utf8"] {
                let mut bad = inverse.clone();
                bad[0][key] = if key == "at" {
                    json!(1)
                } else {
                    json!("unlisted")
                };
                need(
                    config_frame(claim, &physical, "0644", &bad).is_err(),
                    "T07 config altered span accepted",
                )?;
            }
            let mut missing = inverse.clone();
            missing.as_array_mut().unwrap().pop();
            let mut extra = inverse.clone();
            extra.as_array_mut().unwrap().push(inverse[0].clone());
            for bad in [missing, extra] {
                need(
                    config_frame(claim, &physical, "0644", &bad).is_err(),
                    "T07 config missing/duplicate/extra spans accepted",
                )?;
            }
        }

        if selected["current"]["path"] == "crates/xtask/src/main.rs" {
            let parent = super::super::main_isolation_current::parent(
                root,
                "crates/xtask/src/main.rs",
                None,
                super::super::main_isolation_current::Parent::T07,
            )?;
            let inverse: Value =
                serde_json::from_str(MAIN_PARENT_INVERSE).map_err(|e| e.to_string())?;
            need(
                main_frame(&selected["current"], &parent, "0644", &inverse)? == actual,
                "T07 unit-route frame positive differs",
            )?;
            for key in ["path", "sha256", "bytes", "git_blob", "mode"] {
                let mut wrong = selected["current"].clone();
                wrong[key] = json!("unlisted");
                need(
                    main_frame(&wrong, &parent, "0644", &inverse).is_err(),
                    "T07 unit-route changed frame caller accepted",
                )?;
            }
            need(
                main_frame(&selected["current"], &parent, "0600", &inverse).is_err(),
                "T07 unit-route changed frame mode accepted",
            )?;
            let mut bad = parent.clone();
            bad.push(0);
            need(
                main_frame(&selected["current"], &bad, "0644", &inverse).is_err(),
                "T07 unit-route changed parent accepted",
            )?;
            for key in ["at", "remove_utf8", "insert_utf8"] {
                let mut bad = inverse.clone();
                bad[0][key] = if key == "at" {
                    json!(1)
                } else {
                    json!("unlisted")
                };
                need(
                    main_frame(&selected["current"], &parent, "0644", &bad).is_err(),
                    "T07 unit-route changed span accepted",
                )?;
            }
            let mut missing = inverse.clone();
            missing.as_array_mut().unwrap().pop();
            let mut extra = inverse.clone();
            extra.as_array_mut().unwrap().push(inverse[0].clone());
            for bad in [missing, extra] {
                need(
                    main_frame(&selected["current"], &parent, "0644", &bad).is_err(),
                    "T07 unit-route incomplete/extra spans accepted",
                )?;
            }
        }

        let mut wrong = expected.clone();
        wrong["sha256"] = json!("wrong");
        need(
            archived_prior(root, &wrong).is_err(),
            "T07 unit-route wrong predecessor accepted",
        )?;
        let mut bad = actual.clone();
        bad.push(0);
        need(
            validate(selected, &before, &bad, "0644").is_err(),
            "T07 unit-route extra delta accepted",
        )?;
        need(
            validate(selected, &before, &actual, "0755").is_err(),
            "T07 unit-route mode changed",
        )?;
        let mut missing = selected.clone();
        let _ = missing["edits"]
            .as_array_mut()
            .ok_or("T07 unit-route edits")?
            .pop();
        need(
            validate(&missing, &before, &actual, "0644").is_err(),
            "T07 unit-route missing span accepted",
        )?;
        let mut duplicate = selected.clone();
        let first = duplicate["edits"][0].clone();
        duplicate["edits"]
            .as_array_mut()
            .ok_or("T07 unit-route edits")?
            .insert(0, first);
        need(
            validate(&duplicate, &before, &actual, "0644").is_err(),
            "T07 unit-route duplicate span accepted",
        )?;
        let mut span = selected.clone();
        span["edits"][0]["at"] = json!(
            span["edits"][0]["at"]
                .as_u64()
                .ok_or("T07 unit-route offset")?
                + 1
        );
        need(
            validate(&span, &before, &actual, "0644").is_err(),
            "T07 unit-route changed span accepted",
        )?;
        let mut changed = doc.clone();
        changed["transitions"][0]["archive"]["source_path"] = json!("unlisted");
        need(
            document(&serde_json::to_vec(&changed).map_err(|e| e.to_string())?).is_err(),
            "T07 unit-route rehashed/wrong source document accepted",
        )?;
    }
    Ok(())
}
