//! Source-bound native vulnerability-engine qualification; no interpreter/provider.
use crate::{Result, command};
use serde::Deserialize;
use serde_json::{Value, json};
#[derive(Deserialize)]
struct CorpusIdentities {
    records: Vec<CorpusIdentity>,
}
#[derive(Deserialize)]
struct CorpusIdentity {
    id: String,
    #[serde(default, rename = "output")]
    _output: Option<Box<serde_json::value::RawValue>>,
}
fn corpus_ids(root: &Path, path: &str) -> Result<Vec<String>> {
    let bytes = fs::read(root.join(path)).map_err(|e| e.to_string())?;
    let corpus: CorpusIdentities = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    Ok(corpus.records.into_iter().map(|record| record.id).collect())
}
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};
const COVERAGE_SHA: &str = "abd85bf644e311c4f6ed00f1e1315e788a930d937c017ecec7f3befeb378c01a";
fn require(value: bool, message: &str) -> Result<()> {
    if value { Ok(()) } else { Err(message.into()) }
}
fn sha(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn read(root: &Path, path: &str) -> Result<Value> {
    serde_json::from_slice(&fs::read(root.join(path)).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}
fn strings(value: &Value) -> Result<Vec<String>> {
    value
        .as_array()
        .ok_or("A10 string list missing")?
        .iter()
        .map(|v| {
            v.as_str()
                .map(str::to_owned)
                .ok_or("A10 string identity missing".into())
        })
        .collect()
}
fn execution(output: &str, test: &str, multiplicity: usize) -> Result<()> {
    let actual = output
        .lines()
        .filter_map(|line| line.strip_prefix("test ")?.strip_suffix(" ... ok"))
        .collect::<Vec<_>>();
    require(
        actual == vec![test; multiplicity]
            && !output.lines().any(|line| {
                line.starts_with("test ")
                    && (line.ends_with(" ... ignored") || line.ends_with(" ... FAILED"))
            }),
        "A10 exact named native test missing/ignored/failed/extra",
    )?;
    let summaries = output
        .lines()
        .filter(|line| line.starts_with("test result:"))
        .collect::<Vec<_>>();
    require(
        summaries.len() == multiplicity
            && summaries
                .iter()
                .all(|line| line.starts_with("test result: ok. 1 passed; 0 failed; 0 ignored;")),
        "A10 native execution was empty, ignored or a different test inventory",
    )
}
fn markers(output: &str, expected: &[Value]) -> Result<()> {
    let actual = output
        .lines()
        .filter(|line| {
            line.starts_with("A10_")
                && line
                    .split_once(' ')
                    .is_some_and(|(prefix, _)| prefix.ends_with("_COMPLETE"))
        })
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let mut required = expected
        .iter()
        .map(|row| {
            format!(
                "{} {}",
                row["prefix"].as_str().unwrap(),
                row["value"].as_str().unwrap()
            )
        })
        .collect::<Vec<_>>();
    let mut observed = actual.clone();
    required.sort();
    observed.sort();
    require(
        observed == required && observed.iter().collect::<BTreeSet<_>>().len() == observed.len(),
        "A10 case completion missing/duplicate/extra or bound to wrong native test",
    )
}
fn coverage(root: &Path, pack: &Value) -> Result<()> {
    let mut case_ids = BTreeSet::new();
    let mut bindings = BTreeMap::new();
    for test in pack["native_tests"]
        .as_array()
        .ok_or("A10 test bindings missing")?
    {
        let name = test["test"].as_str().ok_or("A10 test name missing")?;
        let target = test["target"].as_str().ok_or("A10 target missing")?;
        require(
            bindings
                .insert((target.to_owned(), name.to_owned()), test)
                .is_none(),
            "A10 duplicate test binding",
        )?;
        let cases = test["cases"].as_array().ok_or("A10 test cases missing")?;
        require(!cases.is_empty(), "A10 native caseless PASS forbidden")?;
        for row in cases {
            let id = row["id"].as_str().ok_or("A10 case ID missing")?;
            require(case_ids.insert(id.to_owned()), "A10 duplicate case ID")?;
            require(
                row["prefix"]
                    .as_str()
                    .is_some_and(|v| v.starts_with("A10_") && v.ends_with("_COMPLETE"))
                    && row["value"].as_str().is_some_and(|v| !v.is_empty()),
                "A10 completion binding missing",
            )?;
        }
    }
    let original = read(root, "docs/rust-rewrite/a10/source-regressions-v1.json")?;
    require(
        original["pytest_exit"] == 0
            && original["source_inputs_unchanged"] == true
            && original["real_network_allowed"] == false
            && original["python"]
                .as_str()
                .is_some_and(|v| v.starts_with("3.14.7 "))
            && original["unicode"] == "16.0.0"
            && original["head"] == pack["source_baseline"],
        "A10 original capture failed/changed/networked",
    )?;
    let calls = original["reports"]
        .as_array()
        .ok_or("A10 original reports missing")?
        .iter()
        .filter(|row| row["when"] == "call")
        .collect::<Vec<_>>();
    require(
        calls.len() == 74 && calls.iter().all(|row| row["outcome"] == "passed"),
        "A10 original74 call identities not completed",
    )?;
    let expected = calls
        .iter()
        .map(|row| row["nodeid"].as_str().unwrap().to_owned())
        .collect::<BTreeSet<_>>();
    require(expected.len() == 74, "A10 duplicate original call identity")?;
    let mappings = pack["source_test_mapping"]
        .as_array()
        .ok_or("A10 source mapping missing")?;
    let actual = mappings
        .iter()
        .map(|row| row["source_test"].as_str().unwrap().to_owned())
        .collect::<BTreeSet<_>>();
    require(
        mappings.len() == 74 && actual == expected,
        "A10 full parameterized74 source mapping changed",
    )?;
    let source = original["records"]
        .as_array()
        .ok_or("A10 original observations missing")?;
    require(source.len() == 133, "A10 original133 scope changed")?;
    for mapping in mappings {
        let source_test = mapping["source_test"].as_str().unwrap();
        let expected = source
            .iter()
            .filter(|row| row["source_test"] == source_test)
            .map(|row| format!("source:baseline133:{}", row["id"].as_str().unwrap()))
            .collect::<BTreeSet<_>>();
        let native = strings(&mapping["native_cases"])?;
        require(
            native.iter().all(|id| case_ids.contains(id)),
            "A10 mapping targets absent native case",
        )?;
        if expected.is_empty() {
            require(
                native == [format!("source-test:{source_test}")],
                "A10 metadata/regex native binding changed",
            )?;
        } else {
            require(
                native.len() == expected.len()
                    && native.into_iter().collect::<BTreeSet<_>>() == expected,
                "A10 behavior mapped to metadata/counter or missing nested helper observations",
            )?;
        }
    }
    for (profile, path, count, prefix) in [
        (
            "baseline133",
            "docs/rust-rewrite/a10/source-regressions-v1.json",
            133,
            "A10_OBSERVATION_COMPLETE",
        ),
        (
            "boundary308",
            "docs/rust-rewrite/a10/source-boundaries-v2.json",
            308,
            "A10_BOUNDARY_COMPLETE",
        ),
        (
            "supplement30",
            "docs/rust-rewrite/a10/source-supplement-v1.json",
            30,
            "A10_BOUNDARY_COMPLETE",
        ),
    ] {
        let rows = corpus_ids(root, path)?;
        require(rows.len() == count, "A10 corpus scope changed")?;
        let mut ids = BTreeSet::new();
        for row in &rows {
            let id = row.as_str();
            require(ids.insert(id), "A10 duplicate corpus identity")?;
            let case_id = format!("source:{profile}:{id}");
            let matching = pack["native_tests"]
                .as_array()
                .unwrap()
                .iter()
                .flat_map(|test| test["cases"].as_array().unwrap())
                .filter(|case| case["id"] == case_id)
                .collect::<Vec<_>>();
            require(
                matching.len() == 1
                    && matching[0]["prefix"] == prefix
                    && matching[0]["value"] == id,
                "A10 full corpus/native binding missing or remapped",
            )?;
        }
    }
    Ok(())
}
// One test-only cleanup suffix changed; the frozen A10 source row remains the caller.
const WORKER_PATH: &str = "crates/ssx-infra/src/html_rows_worker.rs";
const WORKER_PRIOR_SHA: &str = "31194cd4a7d987f6ce106d5f4278337050ac7571625a30c8c5f67edc117b528e";
const WORKER_CURRENT_SHA: &str = "4b3b0e2ad5e0e0e73f683a81a66d218dd0eb329d199d587f87d2688141a278fe";
const WORKER_PRIOR_BLOB: &str = "ed650782fccb5f433d0b5fd30cd6136310e4040b";
const WORKER_CLEANUP_BEFORE: &str = r#"            // Preserve exact command/marker for receipt inspection; only owned
            // fixture files are created and no broad process cleanup is used.
"#;
const WORKER_CLEANUP_AFTER: &str = r#"            // Receipt bytes survive successful fixture retirement in captured stdout.
            // On any earlier failure the controller retains files until child retirement.
            let entered = std::fs::read(&marker).unwrap();
            assert!(
                script.len() <= 8192 && entered.len() <= 128,
                "private fixture receipt bound"
            );
            println!("A05_MEDIA_FIXTURE_RECEIPT command={script:?} entered={entered:?}");
            std::fs::remove_file(&command).unwrap();
            std::fs::remove_file(&marker).unwrap();
            std::fs::remove_dir(&directory).unwrap();
            assert!(
                !directory.exists(),
                "successful owned fixture must be absent"
            );
"#;
fn worker_cleanup_previous(
    container_sha: &str,
    container: &[u8],
    row: &Value,
    current: &[u8],
    mode: u32,
) -> Result<Vec<u8>> {
    require(
        container_sha == COVERAGE_SHA
            && sha(container) == COVERAGE_SHA
            && *row == json!({"path": WORKER_PATH, "sha256": WORKER_PRIOR_SHA}),
        "A10 exact worker cleanup caller differs",
    )?;
    require(
        mode == 0o644 && current.len() == 26237 && sha(current) == WORKER_CURRENT_SHA,
        "A10 exact current worker cleanup bytes/mode differ",
    )?;
    let text = std::str::from_utf8(current).map_err(|e| e.to_string())?;
    require(
        text.matches(WORKER_CLEANUP_AFTER).count() == 1,
        "A10 exact cleanup suffix absent/duplicated",
    )?;
    let prior = text
        .replacen(WORKER_CLEANUP_AFTER, WORKER_CLEANUP_BEFORE, 1)
        .into_bytes();
    require(
        prior.len() == 25633 && sha(&prior) == WORKER_PRIOR_SHA,
        "A10 exact cleanup predecessor differs",
    )?;
    Ok(prior)
}
#[cfg(unix)]
fn worker_cleanup_mode(path: &Path) -> Result<u32> {
    use std::os::unix::fs::PermissionsExt;
    Ok(fs::symlink_metadata(path)
        .map_err(|e| e.to_string())?
        .permissions()
        .mode()
        & 0o7777)
}
#[cfg(not(unix))]
fn worker_cleanup_mode(_: &Path) -> Result<u32> {
    Err("A10 worker cleanup mode proof requires Unix".into())
}
fn check_owned_source(root: &Path, container_sha: &str, row: &Value) -> Result<()> {
    let path = row["path"].as_str().ok_or("A10 source path missing")?;
    if path != WORKER_PATH {
        return crate::a13::check_source_pin(
            root,
            "docs/rust-rewrite/a10/qualification.json",
            container_sha,
            path,
            row["sha256"].as_str().ok_or("A10 source hash missing")?,
            row["bytes"].as_u64().map(|value| value as usize),
        );
    }
    let file = root.join(path);
    let metadata = fs::symlink_metadata(&file).map_err(|e| e.to_string())?;
    require(
        metadata.is_file() && !metadata.file_type().is_symlink(),
        "A10 current worker must be regular non-symlink",
    )?;
    let mode = worker_cleanup_mode(&file)?;
    let current = fs::read(file).map_err(|e| e.to_string())?;
    let container = fs::read(root.join("docs/rust-rewrite/a10/qualification.json"))
        .map_err(|e| e.to_string())?;
    let prior = worker_cleanup_previous(container_sha, &container, row, &current, mode)?;
    let git = std::process::Command::new("git")
        .args([
            "show",
            "e50b32d2e5dc218455a0f361a819c5486c10a8c6:crates/ssx-infra/src/html_rows_worker.rs",
        ])
        .current_dir(root)
        .output()
        .map_err(|e| e.to_string())?;
    require(
        git.status.success() && git.stdout == prior,
        "A10 worker predecessor Git bytes differ",
    )?;
    let tree = std::process::Command::new("git")
        .args([
            "ls-tree",
            "e50b32d2e5dc218455a0f361a819c5486c10a8c6",
            "--",
            WORKER_PATH,
        ])
        .current_dir(root)
        .output()
        .map_err(|e| e.to_string())?;
    require(
        tree.status.success()
            && tree.stdout
                == format!("100644 blob {WORKER_PRIOR_BLOB}\t{WORKER_PATH}\n").as_bytes(),
        "A10 worker predecessor Git mode/blob differ",
    )
}

pub fn check(root: &Path, records: &mut Vec<Value>) -> Result<()> {
    let rust = command(root, "rustc", &["--version", "--verbose"], records)?;
    require(
        rust.starts_with("rustc 1.99.0 ") && rust.lines().any(|line| line == "release: 1.99.0"),
        "A10 requires exact Rust1.99.0",
    )?;
    let path = "docs/rust-rewrite/a10/qualification.json";
    let bytes = fs::read(root.join(path)).map_err(|e| e.to_string())?;
    require(
        sha(&bytes) == COVERAGE_SHA,
        "A10 qualification digest changed",
    )?;
    let pack: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    for row in pack["owned_sources"]
        .as_array()
        .ok_or("A10 owned source bindings missing")?
    {
        check_owned_source(root, &sha(&bytes), row)
            .map_err(|_| "A10 candidate source/fixture/ledger changed".to_owned())?;
    }
    for row in pack["original_sources"]
        .as_array()
        .ok_or("A10 original bindings missing")?
    {
        let path = row["path"].as_str().unwrap();
        let baseline = pack["source_baseline"].as_str().unwrap();
        let bytes = command(
            root,
            "git",
            &["show", &format!("{baseline}:{path}")],
            records,
        )?;
        require(
            sha(bytes.as_bytes()) == row["sha256"].as_str().unwrap()
                && sha(&fs::read(root.join(path)).map_err(|e| e.to_string())?)
                    == row["sha256"].as_str().unwrap(),
            "A10 immutable original Git/source binding changed",
        )?;
    }
    coverage(root, &pack)?;
    command(
        root,
        "cargo",
        &[
            "build",
            "--offline",
            "--locked",
            "-p",
            "ssx-infra",
            "--bin",
            "ssx-html-parser",
        ],
        records,
    )?;
    let mut inventory = BTreeMap::new();
    for target in ["lib", "a10_factories"] {
        let mut args = vec!["test", "--offline", "--locked", "-p", "ssx-infra"];
        if target == "lib" {
            args.push("--lib")
        } else {
            args.extend(["--test", target]);
        }
        args.extend(["--", "--list", "--format", "terse"]);
        let output = command(root, "cargo", &args, records)?;
        let names = output
            .lines()
            .filter_map(|line| line.strip_suffix(": test").map(str::to_owned))
            .collect::<BTreeSet<_>>();
        require(!names.is_empty(), "A10 empty actual test inventory")?;
        inventory.insert(target.to_owned(), names);
    }
    for test in pack["native_tests"].as_array().unwrap() {
        let target = test["target"].as_str().unwrap();
        let name = test["test"].as_str().unwrap();
        require(
            inventory[target].contains(name),
            "A10 exact native test not actually listed",
        )?;
        let mut args = vec!["test", "--offline", "--locked", "-p", "ssx-infra"];
        if target == "lib" {
            args.push("--lib")
        } else {
            args.extend(["--test", target]);
        }
        args.extend([
            name,
            "--",
            "--exact",
            "--nocapture",
            "--format",
            "pretty",
            "--test-threads=2",
        ]);
        let output = command(root, "cargo", &args, records)?;
        execution(
            &output,
            name,
            test["execution_multiplicity"]
                .as_u64()
                .ok_or("A10 execution multiplicity missing")? as usize,
        )?;
        markers(&output, test["cases"].as_array().unwrap())?;
    }
    let name =
        "a10::tests::gate_rejects_missing_ignored_caseless_duplicate_or_wrong_test_completions";
    let output = command(
        root,
        "cargo",
        &[
            "test",
            "--offline",
            "--locked",
            "-p",
            "xtask",
            name,
            "--",
            "--exact",
            "--nocapture",
            "--format",
            "pretty",
            "--test-threads=2",
        ],
        records,
    )?;
    execution(&output, name, 1)?;
    records.push(json!({"gate":"A10","status":"passed","case_count":pack["native_tests"].as_array().unwrap().iter().map(|test|test["cases"].as_array().unwrap().len()).sum::<usize>(),"source_tests":74,"original_observations":133,"original_boundaries":308,"original_supplement":30,"limitations":"Reviewed finite differences and explicit tiny positive real-deadline tuples; no Python/network parity claim beyond recorded source mocks; no production/W09/G03 RSS claim"}));
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn gate_rejects_missing_ignored_caseless_duplicate_or_wrong_test_completions() {
        let test = "actual::named";
        let good = "test actual::named ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0s\n";
        assert!(execution(good, test, 1).is_ok());
        for bad in [
            "",
            "test actual::named ... ignored\ntest result: ok. 0 passed; 0 failed; 1 ignored;",
            "test other::named ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored;",
            "test result: ok. 0 passed; 0 failed; 0 ignored;",
        ] {
            assert!(execution(bad, test, 1).is_err());
        }
        let cases = vec![json!({"prefix":"A10_CASE_COMPLETE","value":"actual-id"})];
        assert!(markers("A10_CASE_COMPLETE actual-id\n", &cases).is_ok());
        for bad in [
            "",
            "A10_CASE_COMPLETE other-id\n",
            "A10_CASE_COMPLETE actual-id\nA10_CASE_COMPLETE actual-id\n",
        ] {
            assert!(markers(bad, &cases).is_err());
        }
        assert!(markers("A10_CASE_COMPLETE actual-id\n", &[]).is_err());
        #[cfg(unix)]
        {
            // This same named control also closes the one finite cleanup predecessor.
            let root = Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap()
                .parent()
                .unwrap();
            let container =
                fs::read(root.join("docs/rust-rewrite/a10/qualification.json")).unwrap();
            let current = fs::read(root.join(WORKER_PATH)).unwrap();
            let row = json!({"path": WORKER_PATH, "sha256": WORKER_PRIOR_SHA});
            check_owned_source(root, COVERAGE_SHA, &row).unwrap();
            assert!(worker_cleanup_previous("wrong", &container, &row, &current, 0o644).is_err());
            let mut bad_container = container.clone();
            bad_container.push(0);
            assert!(
                worker_cleanup_previous(COVERAGE_SHA, &bad_container, &row, &current, 0o644)
                    .is_err()
            );
            for (key, value) in [
                ("path", json!("wrong")),
                ("sha256", json!("wrong")),
                ("bytes", json!(25633)),
            ] {
                let mut changed = row.clone();
                changed[key] = value;
                assert!(
                    worker_cleanup_previous(COVERAGE_SHA, &container, &changed, &current, 0o644)
                        .is_err()
                );
            }
            for mode in [0o600, 0o755, 0o4644] {
                assert!(
                    worker_cleanup_previous(COVERAGE_SHA, &container, &row, &current, mode)
                        .is_err()
                );
            }
            let text = std::str::from_utf8(&current).unwrap();
            let at = text.find(WORKER_CLEANUP_AFTER).unwrap();
            let mut missing = current.clone();
            missing.drain(at..at + WORKER_CLEANUP_AFTER.len());
            let mut duplicate = current.clone();
            duplicate.splice(at..at, WORKER_CLEANUP_AFTER.bytes());
            let mut moved = missing.clone();
            moved.extend_from_slice(WORKER_CLEANUP_AFTER.as_bytes());
            let mut changed = current.clone();
            changed.push(0);
            for raw in [missing, duplicate, moved, changed] {
                assert!(raw != current);
                assert!(
                    worker_cleanup_previous(COVERAGE_SHA, &container, &row, &raw, 0o644).is_err()
                );
            }
        }
    }
}
