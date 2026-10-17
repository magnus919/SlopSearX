//! Native A07 qualification: historical evidence is read-only; no Python runtime.
use crate::{Result, command};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};
const ARTIFACTS: &[(&str, &str)] = &[
    (
        "crates/ssx-infra/tests/support/default_ca_environment.rs",
        "00aee0178f3f111449e5a6e99f495538a1eafa29ec95ced7d8343a3a8ee1108c",
    ),
    (
        "crates/ssx-infra/src/http.rs",
        "700c2a4f4526a97fe54b293c9465262ef94c5666caaae1c0e92f3fd5a64554a6",
    ),
    (
        "crates/ssx-infra/tests/a07_context.rs",
        "b913e5f13290c10b38ea1794b8cae976858ab4951bd0a43b478abb93109eae69",
    ),
    (
        "crates/ssx-infra/tests/support/a07_cold_context.rs",
        "35b0c77f25a4175fd8b25e63f10e2d95d13addde018264f3d5cb4dcf4248c55f",
    ),
    (
        "crates/ssx-infra/tests/support/a07_http_clock.rs",
        "df1b65b18a25f65586e3cc453061b9a78a9a5ce0c1bad9d20885b4df5c99866a",
    ),
    (
        "crates/ssx-infra/tests/support/a07_owner_clock.rs",
        "a3b1f31b273d82e527f587a86e27f7e1d7d972f1bbc9255767a24c632c1f9f4a",
    ),
    (
        "crates/ssx-infra/tests/support/a07_source_replay.rs",
        "f44fa9e3ac288e63bedfad3f41e6f3bc40a0182e35024a45ce3fa3c4311da04a",
    ),
    (
        "crates/ssx-infra/src/http/monotonic.rs",
        "6ea309356076a9dda8f7a313f2dd5f476c624ff28663c91b104bbf86bde63ec6",
    ),
    (
        "docs/rust-rewrite/a07/cases.json",
        "2970b172ad13b44ea55e502cddf2a6c2ec0f18eb75eb6eadfe095d2e586067b9",
    ),
    (
        "docs/rust-rewrite/a07/consumer-fixture-provenance.json",
        "b5b8239ccdfdd0febc52d1eeddf83acd22ab6b0b381c684853cf6b3b16a9bf9c",
    ),
    (
        "docs/rust-rewrite/a07/coverage.json",
        "be6826b82c155617d02ce94bc48a0ddedbc75abdd535477f2c7889c63676661b",
    ),
    (
        "docs/rust-rewrite/a07/fixtures/clinicaltrials-success.json",
        "b7049fd602eed804929d7173c147e2730035db24ef0e17f0eae1cc4bb92f155e",
    ),
    (
        "docs/rust-rewrite/a07/fixtures/europepmc-biomedical.json",
        "2c9a29ad299b578a8d1fc16db5ebef0ad0b66333bfb5c4878bfc209ce8e87f96",
    ),
    (
        "docs/rust-rewrite/a07/fixtures/europepmc-long-preprint.json",
        "8f10cfd306772844976d428ec9a3e778db1a559bb067c147e9b708436fc2762e",
    ),
    (
        "docs/rust-rewrite/a07/fixtures/europepmc-preprints.json",
        "2d1115ea10cfbd1a902d60501d33471a0ee800f01f27b9e7c891591e636a5ee1",
    ),
    (
        "docs/rust-rewrite/a07/fixtures/pubchem-detail.json",
        "9fa91ac2c90b51922a5e758e47a0baf62ceac4cb41bfb62f3ce58647d6665075",
    ),
    (
        "docs/rust-rewrite/a07/fixtures/pubchem-search.json",
        "b9822f4ee31a842d19aa9abc5b69874c97e44acee78b3d896be7e0031ef45442",
    ),
    (
        "docs/rust-rewrite/a07/fixtures/pubmed-search.json",
        "df1cdc5ef7bb4079407e0334aecf1c48c2ad54ac12d77619b8f10fa64633ff48",
    ),
    (
        "docs/rust-rewrite/a07/fixtures/pubmed-summary.json",
        "f0e7ad77d6e30b78e8eac2d74b07f0fe0981d88c6ca308eccf6ac43de095a190",
    ),
    (
        "docs/rust-rewrite/a07/fixtures/uniprot-success.json",
        "8dfd341cdc78d85726bdaff3c0628dc24b282cb06f6e194fed105bdfe78f7fe0",
    ),
    (
        "docs/rust-rewrite/a07/proposed-differences.json",
        "a0445640b0debd898476fa8680520a263ba6e9c1f450617ef991cd813a304e01",
    ),
    (
        "docs/rust-rewrite/a07/resource-bodies/ct-large-one-row-0.json",
        "76f820fe6ae2fdeffc836f7f41c1717cb35f42066857cd738969a2cec11727da",
    ),
    (
        "docs/rust-rewrite/a07/resource-bodies/ep-1000-realistic-0.json",
        "9c1329db236691e4bca093eb39c144dcb1234ce2f26da1cebb8f2b5475ed5de5",
    ),
    (
        "docs/rust-rewrite/a07/resource-bodies/ep-large-one-row-0.json",
        "b261794076819ce108e0ee2940a1bb0072fb399b60f1a11141457e0cbef9ad4c",
    ),
    (
        "docs/rust-rewrite/a07/resource-bodies/manifest.json",
        "7a30a50648787a0f7c741e91fb027256559cf7abaf5f9852f038f11489c1415d",
    ),
    (
        "docs/rust-rewrite/a07/resource-bodies/pc-large-one-row-0.json",
        "63535f1fca37e9da176550cd965bbc04f5eec182ab5031451ef075386ee1d4c5",
    ),
    (
        "docs/rust-rewrite/a07/resource-bodies/pc-large-one-row-1.json",
        "7281069d904686961ccadb59371d98ec039f1594247f7c7321d9c2cee98fcb17",
    ),
    (
        "docs/rust-rewrite/a07/resource-bodies/pm-200-realistic-0.json",
        "53fba19be56ad756fe85d77e15b58c3f8733b12d9ca6e612dca64aaeb47f51f0",
    ),
    (
        "docs/rust-rewrite/a07/resource-bodies/pm-200-realistic-1.json",
        "2df1046cc14033e6d61349368f894293b9cb91a6032806e4c1611ac054286583",
    ),
    (
        "docs/rust-rewrite/a07/resource-bodies/pm-large-one-row-0.json",
        "9ba447af5303a8c02e757f0b566395a1d954a05e28b7cbd0368c06decb6e25cf",
    ),
    (
        "docs/rust-rewrite/a07/resource-bodies/pm-large-one-row-1.json",
        "13a748012b8e63e6c0b67121b13ce47dacc9c91c3c63b5ca67af2620aeac948c",
    ),
    (
        "docs/rust-rewrite/a07/resource-bodies/pm-surrogate-author-after-display-0.json",
        "9ba447af5303a8c02e757f0b566395a1d954a05e28b7cbd0368c06decb6e25cf",
    ),
    (
        "docs/rust-rewrite/a07/resource-bodies/pm-surrogate-author-after-display-1.json",
        "1cd0a1bb00dbee0314d50979d5293a485a718b7cb3a06cda06bdda181887f67a",
    ),
    (
        "docs/rust-rewrite/a07/resource-bodies/up-large-one-row-0.json",
        "5ed5b996f376f2c89b65b93b0a0a88335c0a5020d22cf5d34e890f0dd8733b19",
    ),
    (
        "docs/rust-rewrite/a07/source-repr-controls.json",
        "8facd8f7d7814c31dec430c7ffd42034d7fb30414fdc44148afcd1ccde865f3c",
    ),
    (
        "docs/rust-rewrite/a07/source_complete.json",
        "d8c8a3910abafe63426c9ca1c9a62640ed5d3544442b407e1adbdcbefbb34314",
    ),
    (
        "docs/rust-rewrite/a07/source_date_grammar.json",
        "86d3e699b6a7fc47304d462b99bd919972453508c5516ee366fb4015b580c379",
    ),
    (
        "docs/rust-rewrite/a07/source_digits.json",
        "975cc5bfcf0e5a569c77ab1d85ffe5cf673b759f329e0dba59c92aab5381b217",
    ),
    (
        "docs/rust-rewrite/a07/source_empty_iterables.json",
        "b0c002ad908349a6d434f3c10c346535be563db9ba85f35f7f511c7291269abd",
    ),
    (
        "docs/rust-rewrite/a07/source_gene_shapes.json",
        "b87190f00b76af47c6cb61d31a687743f28bd027f8f42a5df78c676a142b3616",
    ),
    (
        "docs/rust-rewrite/a07/source_matrix.json",
        "c5963884730a7a65454bf2c430d6f405f532f9cab6913d76b0ac3e9a1bc2b927",
    ),
    (
        "docs/rust-rewrite/a07/source_numeric.json",
        "9ebb40789c044dbd65305cfea9f798ca950a852382563282dd3c2f1de7ba5888",
    ),
    (
        "docs/rust-rewrite/a07/source_sequences.json",
        "f69749b50a6ce6b63034721884813693ec9b48319afa491c42d8bdbc221df4f8",
    ),
    (
        "docs/rust-rewrite/a07/source_supplement.json",
        "2ff2c54f04af1700cdeaac3df19927120a39a4808177470dbe41a6d27dffa8c4",
    ),
    (
        "docs/rust-rewrite/a07/source_timeout_loopback.json",
        "e9e633690d0f43c7f1a93f313540c10faf24653e9aa81a59fcb9169b3b3ce1db",
    ),
    (
        "docs/rust-rewrite/a06/source-publication-envelope.json",
        "407256b5dc7d5320389f7754a75f230212a11cac2971346d5609539330581e0a",
    ),
    (
        "docs/rust-rewrite/a06/source-payload-controls.json",
        "293796c517e7c6f41d9c427e76f2af1f892edd208f276a93af305a63b6d0e69e",
    ),
];

fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
fn require(ok: bool, reason: &str) -> Result<()> {
    if ok { Ok(()) } else { Err(reason.into()) }
}
fn verify(root: &Path, artifacts: &[(&str, &str)]) -> Result<()> {
    for (path, sha) in artifacts {
        let bytes =
            fs::read(root.join(path)).map_err(|e| format!("A07 missing artifact {path}: {e}"))?;
        require(
            digest(&bytes) == *sha,
            &format!("A07 changed artifact: {path}"),
        )?;
    }
    Ok(())
}
fn insert_primary(ids: &mut BTreeSet<String>, row: &Value) -> Result<()> {
    let id = row["id"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or("A07 missing primary ID")?;
    require(ids.insert(id.to_owned()), "A07 duplicate primary ID")?;
    require(
        row.get("output").is_some() && row.get("requests").is_some(),
        "A07 incomplete source outcome/request control",
    )
}
fn primary_ids(root: &Path) -> Result<BTreeSet<String>> {
    let mut ids = BTreeSet::new();
    for name in [
        "source_complete",
        "source_gene_shapes",
        "source_date_grammar",
        "source_sequences",
        "source_empty_iterables",
    ] {
        let doc: Value = serde_json::from_slice(
            &fs::read(root.join(format!("docs/rust-rewrite/a07/{name}.json")))
                .map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        for row in doc["records"]
            .as_array()
            .ok_or("A07 missing primary records")?
        {
            insert_primary(&mut ids, row)?;
        }
    }
    require(ids.len() == 172, "A07 primary control cardinality changed")?;
    Ok(ids)
}
fn exact_test_present(
    binding: &Value,
    available: &BTreeMap<(String, String), BTreeSet<String>>,
) -> Result<()> {
    let package = binding["package"]
        .as_str()
        .ok_or("A07 native package missing")?;
    let target = binding["target"]
        .as_str()
        .ok_or("A07 native target missing")?;
    let name = binding["qualified_name"]
        .as_str()
        .filter(|n| !n.is_empty())
        .ok_or("A07 qualified native name missing")?;
    require(
        available
            .get(&(package.to_owned(), target.to_owned()))
            .is_some_and(|names| names.contains(name)),
        &format!("A07 missing exact native identity {package}/{target}/{name}"),
    )
}
fn verify_executed(output: &str, expected: &BTreeSet<String>) -> Result<()> {
    let passed: BTreeSet<String> = output
        .lines()
        .filter_map(|line| {
            line.strip_prefix("test ")?
                .strip_suffix(" ... ok")
                .map(str::to_owned)
        })
        .collect();
    require(
        passed == *expected,
        "A07 listed native tests were missing, ignored, or did not pass",
    )
}
const MOVED_FULL: [(&str, &str); 4] = [
    (
        "clinicaltrials-config-timeout_ms-bool-True",
        "clinicaltrials_bool_true_full_source_replay",
    ),
    (
        "europepmc-config-timeout_ms-bool-True",
        "europepmc_bool_true_full_source_replay",
    ),
    (
        "pubchem-config-timeout_ms-bool-True",
        "pubchem_bool_true_full_source_replay",
    ),
    (
        "uniprot-config-timeout_ms-bool-True",
        "uniprot_bool_true_full_source_replay",
    ),
];
fn verify_replay_receipts(bytes: &str, expected: &BTreeSet<String>) -> Result<()> {
    let mut actual = BTreeSet::new();
    for line in bytes.lines() {
        let row: Value = serde_json::from_str(line).map_err(|e| e.to_string())?;
        require(
            row["kind"] == "full_public_SearchEngine"
                && row["compared"] == "original_config_status_requests_complete_output",
            "A07 incomplete full-adapter receipt",
        )?;
        let id = row["id"]
            .as_str()
            .filter(|id| !id.is_empty())
            .ok_or("A07 missing replay receipt ID")?;
        require(
            actual.insert(id.to_owned()),
            "A07 duplicate executed source ID",
        )?;
    }
    require(
        actual == *expected,
        "A07 exact full-adapter172 source ID union was not executed",
    )
}
struct ReplayReceipts(std::path::PathBuf);
impl ReplayReceipts {
    fn fresh() -> Result<Self> {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "a07-full-replay-{}-{stamp}.jsonl",
            std::process::id()
        ));
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|e| e.to_string())?;
        Ok(Self(path))
    }
}
impl Drop for ReplayReceipts {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}
pub fn check(root: &Path, records: &mut Vec<Value>) -> Result<()> {
    verify(root, ARTIFACTS)?;
    let ids = primary_ids(root)?;
    let coverage: Value = serde_json::from_slice(
        &fs::read(root.join("docs/rust-rewrite/a07/coverage.json")).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    require(
        coverage["case_count"] == 172,
        "A07 coverage primary count changed",
    )?;
    // Historical native/CI pins are checked against exact baseline Git blobs.
    // Live original provider/config/test bindings below remain exact current bytes.
    for source in coverage["source_inventory"]
        .as_array()
        .ok_or("A07 source inventory missing")?
    {
        let path = source["path"].as_str().ok_or("A07 malformed source path")?;
        if path.starts_with("crates/") || path == "docs/rust-rewrite/ci/gates.json" {
            let blob = source["git_blob"]
                .as_str()
                .ok_or("A07 historical blob identity missing")?;
            require(
                blob.len() == 40 && blob.bytes().all(|b| b.is_ascii_hexdigit()),
                "A07 invalid historical blob identity",
            )?;
            let original = command(root, "git", &["cat-file", "blob", blob], records)?;
            require(
                digest(original.as_bytes())
                    == source["sha256"]
                        .as_str()
                        .ok_or("A07 historical hash missing")?,
                &format!("A07 historical native/CI blob changed: {path}"),
            )?;
            records.push(json!({"A07_source_binding":{"path":path,"kind":"historical baseline native/CI blob; current native code qualified by actual tests","git_blob":blob}}));
            continue;
        }
        require(
            digest(&fs::read(root.join(path)).map_err(|e| e.to_string())?)
                == source["sha256"]
                    .as_str()
                    .ok_or("A07 malformed source hash")?,
            &format!("A07 original source changed: {path}"),
        )?;
    }
    let receipts = ReplayReceipts::fresh()?;
    let mut available = BTreeMap::new();
    for (package, target) in [
        ("ssx-infra", "lib"),
        ("ssx-infra", "test:a07_biomedical"),
        ("ssx-infra", "test:a07_context"),
        ("ssx-infra", "test:a07_factories"),
        ("ssx-mcp", "test:a07_consumers"),
    ] {
        let mut args = vec!["test", "--offline", "-p", package];
        if target == "lib" {
            args.push("--lib");
        } else {
            args.extend([
                "--test",
                target
                    .strip_prefix("test:")
                    .ok_or("A07 invalid binary binding")?,
            ]);
        }
        args.extend(["--", "--list"]);
        let listed = command(root, "cargo", &args, records)?;
        let names = listed
            .lines()
            .filter_map(|line| line.strip_suffix(": test").map(str::to_owned))
            .collect::<BTreeSet<_>>();
        require(!names.is_empty(), "A07 native binary contains no tests")?;
        available.insert((package.to_owned(), target.to_owned()), names);
    }
    let migrated = coverage["full_source_execution"]["moved"]
        .as_array()
        .ok_or("A07 full replay migration missing")?;
    require(
        migrated.len() == 4
            && coverage["full_source_execution"]["total"] == 172
            && coverage["full_source_execution"]["remaining"]["count"] == 168,
        "A07 full replay partition changed",
    )?;
    for (id, name) in MOVED_FULL {
        let matching = migrated
            .iter()
            .filter(|row| row["id"] == id)
            .collect::<Vec<_>>();
        require(
            matching.len() == 1,
            "A07 missing/duplicate migrated source ID",
        )?;
        let binding = matching[0];
        require(
            binding["package"] == "ssx-infra"
                && binding["target"] == "lib"
                && binding["qualified_name"]
                    == format!("adapters::clinicaltrials::owner_clock_tests::{name}"),
            "A07 wrong full-adapter migration identity",
        )?;
        exact_test_present(binding, &available)?;
    }
    for mapping in coverage["regression_mapping"]
        .as_array()
        .ok_or("A07 mappings missing")?
    {
        require(
            mapping["native_cases"]
                .as_array()
                .is_some_and(|v| !v.is_empty()),
            "A07 unmapped original regression",
        )?;
        require(
            mapping["mapping_status"]
                .as_str()
                .is_some_and(|s| !s.is_empty()),
            "A07 missing mapping disposition",
        )?;
        if mapping["live_forbidden"] == true {
            require(
                mapping["mapping_status"] == "deferred live qualification",
                "A07 unauthorized live disposition",
            )?;
            continue;
        }
        for case in mapping["native_cases"]
            .as_array()
            .ok_or("A07 native cases missing")?
        {
            let key = case
                .as_str()
                .ok_or("A07 native test binding key malformed")?;
            let binding = coverage["native_case_bindings"]
                .get(key)
                .ok_or("A07 native identity binding missing")?;
            exact_test_present(binding, &available)?;
        }
        let path = mapping["path"]
            .as_str()
            .ok_or("A07 missing original test path")?;
        require(
            root.join(path).is_file(),
            "A07 missing original regression consumer",
        )?;
    }
    command(
        root,
        "cargo",
        &["test", "--offline", "-p", "xtask", "a07::tests"],
        records,
    )?;
    for ((package, target), expected) in &available {
        let mut args = vec!["test", "--offline", "-p", package.as_str()];
        if target == "lib" {
            args.push("--lib");
        } else {
            args.extend([
                "--test",
                target
                    .strip_prefix("test:")
                    .ok_or("A07 invalid binary binding")?,
            ]);
        }
        args.extend(["--", "--format", "pretty"]);
        // Only test evidence output is configured; Manual clocks have no env path.
        let receipt_arg = format!("A07_COMPLETED_CASES={}", receipts.0.display());
        let mut run_args = vec![receipt_arg.as_str(), "cargo"];
        run_args.extend(args.iter().copied());
        let output = command(root, "env", &run_args, records)?;
        verify_executed(&output, expected)?;
    }
    verify_replay_receipts(
        &fs::read_to_string(&receipts.0).map_err(|e| e.to_string())?,
        &ids,
    )?;
    records.push(json!({"A07":{"primary_source_controls":ids.len(),"factory_availability":"actual Registry attachment, separate from historical F06 declaration","temporary_python":"not executed or required","remaining_boundaries":"G02 retained-response/global input accounting and full MCP/HTML transport remain separate"}}));
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn full_replay_receipts_reject_missing_duplicate_wrong_kind_and_extra_ids() {
        let expected = BTreeSet::from(["one".to_owned()]);
        let row=json!({"id":"one","kind":"full_public_SearchEngine","compared":"original_config_status_requests_complete_output"}).to_string();
        assert!(verify_replay_receipts(&row, &expected).is_ok());
        assert!(verify_replay_receipts("", &expected).is_err());
        assert!(verify_replay_receipts(&format!("{row}\n{row}"), &expected).is_err());
        assert!(
            verify_replay_receipts(
                &row.replace("full_public_SearchEngine", "semantic_only"),
                &expected
            )
            .is_err()
        );
        assert!(verify_replay_receipts(&row, &BTreeSet::new()).is_err());
    }
    #[test]
    fn ignored_missing_and_wrong_identity_execution_fail_closed() {
        let expected = BTreeSet::from(["module::required".to_owned()]);
        verify_executed("test module::required ... ok\n", &expected).unwrap();
        for output in [
            "test module::required ... ignored\n",
            "test result: ok. 0 passed; 0 failed; 1 ignored\n",
            "test other::required ... ok\n",
            "",
        ] {
            assert!(verify_executed(output, &expected).is_err());
        }
    }
    #[test]
    fn same_suffix_in_wrong_module_binary_or_package_is_rejected() {
        let available = BTreeMap::from([(
            ("ssx-infra".into(), "lib".into()),
            BTreeSet::from(["correct::same_name".into()]),
        )]);
        exact_test_present(
            &json!({"package":"ssx-infra","target":"lib","qualified_name":"correct::same_name"}),
            &available,
        )
        .unwrap();
        for binding in [
            json!({"package":"ssx-infra","target":"lib","qualified_name":"wrong::same_name"}),
            json!({"package":"ssx-infra","target":"test:a07_biomedical","qualified_name":"correct::same_name"}),
            json!({"package":"ssx-mcp","target":"lib","qualified_name":"correct::same_name"}),
            json!({"package":"ssx-infra","target":"lib","qualified_name":"same_name"}),
        ] {
            assert!(exact_test_present(&binding, &available).is_err());
        }
    }
    #[test]
    fn duplicate_missing_and_incomplete_primary_controls_fail_closed() {
        let mut ids = BTreeSet::new();
        let row = json!({"id":"one","output":{},"requests":[]});
        insert_primary(&mut ids, &row).unwrap();
        assert!(insert_primary(&mut ids, &row).is_err());
        assert!(insert_primary(&mut ids, &json!({"id":"","output":{},"requests":[]})).is_err());
        assert!(insert_primary(&mut ids, &json!({"id":"two","output":{}})).is_err());
    }
    #[test]
    fn nonzero_native_command_fails_qualification() {
        let mut records = Vec::new();
        assert!(
            command(
                Path::new("."),
                "rustc",
                &["--a07-deliberately-invalid-option"],
                &mut records
            )
            .is_err()
        );
        assert_ne!(records[0]["exit_code"], 0);
    }
    #[test]
    fn missing_and_tampered_artifacts_fail_closed() {
        let root = std::env::temp_dir().join(format!("a07-gate-probe-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let pin = digest(b"fixed");
        assert!(verify(&root, &[("fixture", &pin)]).is_err());
        fs::write(root.join("fixture"), b"tampered").unwrap();
        assert!(verify(&root, &[("fixture", &pin)]).is_err());
        fs::write(root.join("fixture"), b"fixed").unwrap();
        verify(&root, &[("fixture", &pin)]).unwrap();
        fs::remove_dir_all(root).unwrap();
    }
}
