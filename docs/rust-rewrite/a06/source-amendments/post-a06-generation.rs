//! Closed T02 generations after A06. Historical operands never become current identity.
use serde_json::{Value, json};
use std::{fs, path::Path, process::Command};
#[path = "native-search-reader.rs"]
pub(crate) mod native_search;
const CAPSULE: &str = "docs/rust-rewrite/a06/source-amendments/post-a06-generation-v1.json";
const SHA: &str = "3ff125fdcbca12bf6312ef875a613360319fdd11349fd2524a0fb932f31979cd";
fn document(raw: &[u8]) -> Result<Value, String> {
    super::need(super::digest(raw) == SHA, "T02 generation capsule changed")?;
    let doc: Value = serde_json::from_slice(raw).map_err(|e| e.to_string())?;
    super::need(
        doc["schema"] == "T02_POST_A06_CLOSED_GENERATION_V1"
            && doc["source_parent"] == "233c7934f9e2da5c2e94f0c1213945335d786502"
            && doc["continuation_head"] == "4de0d641ea6e80b398f2cc12b46b2c818608bbac"
            && doc["rows"].as_array().is_some_and(|r| r.len() == 24)
            && doc["callers"].as_array().is_some_and(|r| r.len() == 77),
        "T02 generation closed membership",
    )?;
    Ok(doc)
}
fn load(root: &Path) -> Result<Value, String> {
    document(&super::read_exact(
        root,
        &json!({"path":CAPSULE,"sha256":SHA,"bytes":126027,"mode":"0644"}),
    )?)
}
fn tuple(expected: &Value, sealed: &Value) -> bool {
    expected["path"] == sealed["path"]
        && expected["sha256"] == sealed["sha256"]
        && expected["bytes"] == sealed["bytes"]
        && expected["mode"] == sealed["mode"]
}
fn find<'a>(doc: &'a Value, expected: &Value) -> Option<&'a Value> {
    doc["rows"]
        .as_array()?
        .iter()
        .find(|r| tuple(expected, &r["previous"]))
}
fn payload(row: &Value, prior: &[u8], current: &[u8], mode: &str) -> Result<(), String> {
    super::material(&row["previous"], prior, "0644")?;
    super::material(&row["current"], current, mode)?;
    super::need(
        row["previous"]["path"] == row["current"]["path"]
            && row["archive"]["sha256"] == row["previous"]["sha256"]
            && row["archive"]["bytes"] == row["previous"]["bytes"]
            && row["archive"]["mode"] == "0644"
            && super::edits(prior, &row["edits"])? == current,
        "T02 generation exact ordered reconstruction",
    )
}
fn verify(root: &Path, row: &Value) -> Result<(Vec<u8>, Vec<u8>), String> {
    if row["previous"]["path"] == native_search::SOURCE
        && row["previous"]["sha256"]
            == "fcac2929eb6637960d5e1086c321f0eac62fc0f8446cf73f22d6b43406137b12"
    {
        return native_search::materials_for_row(root, row);
    }
    let prior = super::read_exact(root, &row["archive"])?;
    let current = super::read_exact(root, &row["current"])?;
    payload(
        row,
        &prior,
        &current,
        &super::permissions(&root.join(super::text(&row["current"], "path")?))?,
    )?;
    let spec = format!(
        "{}:{}",
        super::text(row, "source_commit")?,
        super::text(&row["previous"], "path")?
    );
    let output = Command::new("git")
        .args(["show", &spec])
        .current_dir(root)
        .output()
        .map_err(|e| e.to_string())?;
    super::need(
        output.status.success() && output.stdout == prior,
        "T02 exact historical Git bytes",
    )?;
    let commit = super::text(row, "source_commit")?;
    let source = super::text(&row["previous"], "path")?;
    let tree = Command::new("git")
        .args(["ls-tree", commit, "--", source])
        .current_dir(root)
        .output()
        .map_err(|e| e.to_string())?;
    let expected_tree = format!(
        "100644 blob {}\t{}\n",
        super::text(row, "source_blob")?,
        source
    );
    super::need(
        tree.status.success() && tree.stdout == expected_tree.as_bytes(),
        "T02 exact historical Git tree mode/blob",
    )?;
    if let Some(capsule) = row["t02_capsule"].as_str() {
        let raw = fs::read(root.join(capsule)).map_err(|e| e.to_string())?;
        super::need(
            row["t02_capsule_sha256"] == super::digest(&raw),
            "T02 existing two-source authority changed",
        )?;
        let doc: Value = serde_json::from_slice(&raw).map_err(|e| e.to_string())?;
        let t = doc["transitions"]
            .as_array()
            .ok_or("T02 two-source rows absent")?
            .iter()
            .find(|t| t["path"] == row["current"]["path"])
            .ok_or("T02 two-source binding absent")?;
        super::need(
            t["old_sha256"] == row["previous"]["sha256"]
                && t["current_sha256"] == row["current"]["sha256"]
                && t["edits"] == row["t02_edits"],
            "T02 two-source exact linkage",
        )?;
    }
    Ok((prior, current))
}
/// A selected exact historical tuple is returned only after physical current proof.
pub(super) fn historical(root: &Path, expected: &Value) -> Result<Option<Vec<u8>>, String> {
    let doc = load(root)?;
    let Some(row) = find(&doc, expected) else {
        return Ok(None);
    };
    Ok(Some(verify(root, row)?.0))
}
pub(super) fn effective(root: &Path, expected: &Value) -> Result<Value, String> {
    let doc = load(root)?;
    match find(&doc, expected) {
        Some(row) => {
            verify(root, row)?;
            Ok(row["current"].clone())
        }
        None => Ok(expected.clone()),
    }
}
pub(super) fn actual(root: &Path, expected: &Value) -> Result<Vec<u8>, String> {
    super::read_exact(root, &effective(root, expected)?)
}
/// Exact caller material for the unchanged historical scoped controls.
pub(crate) fn caller_material(
    root: &Path,
    path: &str,
    expected_sha: &str,
) -> Result<Vec<u8>, String> {
    let doc = load(root)?;
    if let Some(row) = doc["rows"]
        .as_array()
        .ok_or("T02 rows absent")?
        .iter()
        .find(|r| r["previous"]["path"] == path && r["previous"]["sha256"] == expected_sha)
    {
        return Ok(verify(root, row)?.0);
    }
    super::read_exact(
        root,
        &json!({"path":path,"sha256":expected_sha,"bytes":fs::metadata(root.join(path)).map_err(|e|e.to_string())?.len(),"mode":"0644"}),
    )
}
fn caller(
    binding: &Value,
    hash: &str,
    source: &str,
    expected: &str,
    bytes: Option<usize>,
) -> Result<(), String> {
    super::need(
        binding["current_sha256"] == hash
            && binding["source_path"] == source
            && binding["current_digest"] == expected
            && binding["current_bytes"] == json!(bytes),
        "T02 exact current-generation caller",
    )
}
pub(super) fn try_current(
    root: &Path,
    container: &str,
    hash: &str,
    source: &str,
    expected: &str,
    bytes: Option<usize>,
) -> Option<Result<(), String>> {
    // Historical routes remain unchanged. Current generations have their own finite matrix.
    if ![
        "docs/rust-rewrite/a09/coverage.json",
        "docs/rust-rewrite/a10/qualification.json",
        "docs/rust-rewrite/a11/qualification.json",
        "docs/rust-rewrite/a12/qualification-v1.json",
        "docs/rust-rewrite/a13/qualification-v1.json",
        "docs/rust-rewrite/a13/dehashed-source-map-v1.json",
        "docs/rust-rewrite/a13/remaining/source-map.json",
        "docs/rust-rewrite/o01/coverage.json",
        "docs/rust-rewrite/w06/qualification.json",
    ]
    .contains(&container)
    {
        return None;
    }
    let doc = match load(root) {
        Ok(d) => d,
        Err(e) => return Some(Err(e)),
    };
    let binding = doc["callers"]
        .as_array()?
        .iter()
        .find(|b| b["container"] == container && b["source_path"] == source)?;
    if binding["current_sha256"] != hash
        && (binding["previous_sha256"] == hash || binding["previous_digest"] == expected)
    {
        return None;
    }
    Some((|| {
        caller(binding, hash, source, expected, bytes)?;
        let metadata = doc["rows"]
            .as_array()
            .ok_or("T02 rows absent")?
            .iter()
            .find(|r| r["current"]["path"] == container)
            .ok_or("T02 current container absent")?;
        let (_, current) = verify(root, metadata)?;
        super::need(
            super::digest(&current) == hash,
            "T02 actual caller identity",
        )?;
        let current: Value = serde_json::from_slice(&current).map_err(|e| e.to_string())?;
        let pin = current
            .pointer(super::text(binding, "pointer")?)
            .ok_or("T02 exact caller pointer absent")?;
        super::need(
            (pin.is_object()
                && pin["path"] == source
                && pin["sha256"] == expected
                && pin.get("bytes").cloned().unwrap_or(Value::Null) == json!(bytes))
                || (pin == expected && bytes.is_none()),
            "T02 current caller row differs",
        )?;
        let old_hash = super::text(binding, "previous_sha256")?;
        let old_expected = super::text(binding, "previous_digest")?;
        let old_bytes = binding["previous_bytes"]
            .as_u64()
            .and_then(|n| usize::try_from(n).ok());
        let route = binding["predecessor_route"]
            .as_str()
            .unwrap_or(super::text(binding, "route")?);
        if route == "shared" {
            super::try_verify(root, container, old_hash, source, old_expected, old_bytes)
                .ok_or("T02 original shared route absent")??;
        } else if route == "scoped" {
            super::super::try_verify(root, container, old_hash, source, old_expected, old_bytes)
                .ok_or("T02 original scoped route absent")??;
        }
        if route == "factory" {
            super::super::verify_current(
                root,
                container,
                old_hash,
                source,
                old_expected,
                old_bytes,
            )?;
        }
        if binding["route"] == "direct" {
            if let Some(row) = doc["rows"]
                .as_array()
                .ok_or("T02 rows absent")?
                .iter()
                .find(|r| r["previous"]["path"] == source && r["previous"]["sha256"] == expected)
            {
                let (prior, _) = verify(root, row)?;
                super::need(
                    bytes.is_none_or(|n| n == prior.len()),
                    "T02 direct source caller bytes",
                )?;
            } else {
                let raw = super::read_exact(
                    root,
                    &json!({"path":source,"sha256":expected,"bytes":fs::metadata(root.join(source)).map_err(|e|e.to_string())?.len(),"mode":"0644"}),
                )?;
                super::need(
                    bytes.is_none_or(|n| n == raw.len()),
                    "T02 direct current caller bytes",
                )?;
            }
        }
        Ok(())
    })())
}
pub(super) fn scoped(root: &Path, row: &Value, historical: &[u8]) -> Result<Value, String> {
    let doc = load(root)?;
    let Some(successor) = find(&doc, &row["current"]) else {
        return Ok(row.clone());
    };
    let (prior, current) = verify(root, successor)?;
    super::material(&successor["scoped_historical"], historical, "0644")?;
    let byte_edits = |v: &Value| -> Result<Value, String> {
        Ok(json!(v.as_array().ok_or("T02 scoped edits absent")?.iter().map(|e| Ok(json!({"at":e["at"],"remove":super::text(e,"remove_utf8")?.as_bytes(),"insert":super::text(e,"insert_utf8")?.as_bytes()}))).collect::<Result<Vec<Value>,String>>()?))
    };
    // The pre-existing scoped reconstruction is independently checked before advancement.
    let utf8: Vec<Value> = row["edits"].as_array().ok_or("T02 prior scoped edits absent")?.iter().map(|e| {
        let a: Vec<u8> = serde_json::from_value(e["remove"].clone()).map_err(|x| x.to_string())?;
        let b: Vec<u8> = serde_json::from_value(e["insert"].clone()).map_err(|x| x.to_string())?;
        Ok(json!({"at":e["at"],"remove_utf8":String::from_utf8(a).map_err(|x|x.to_string())?,"insert_utf8":String::from_utf8(b).map_err(|x|x.to_string())?}))
    }).collect::<Result<_, String>>()?;
    super::need(
        super::edits(historical, &json!(utf8))? == prior
            && super::edits(historical, &successor["scoped_edits"])? == current,
        "T02 full scoped current reconstruction",
    )?;
    let mut next = row.clone();
    next["current"] = successor["current"].clone();
    next["edits"] = byte_edits(&successor["scoped_edits"])?;
    Ok(next)
}
#[cfg(test)]
pub(super) fn controls(root: &Path) -> Result<(), String> {
    let doc = load(root)?;
    for row in doc["rows"].as_array().ok_or("T02 rows absent")? {
        let (prior, current) = verify(root, row)?;
        payload(row, &prior, &current, "0644")?;
        for mode in ["0600", "0755", "4644"] {
            super::need(
                payload(row, &prior, &current, mode).is_err(),
                "T02 mode accepted",
            )?;
        }
        let mut changed = current.clone();
        changed.push(0);
        super::need(
            payload(row, &prior, &changed, "0644").is_err(),
            "T02 unrelated current accepted",
        )?;
        super::need(
            payload(row, &current, &current, "0644").is_err(),
            "T02 current as historical accepted",
        )?;
        let mut wrong = row["previous"].clone();
        wrong["sha256"] = json!("wrong");
        super::need(
            find(&doc, &wrong).is_none(),
            "T02 unknown historical hash accepted",
        )?;
        let mut rehashed = row.clone();
        rehashed["current"]["sha256"] = json!(super::digest(&changed));
        rehashed["current"]["bytes"] = json!(changed.len());
        super::need(
            payload(&rehashed, &prior, &changed, "0644").is_err(),
            "T02 rehashed unrelated current accepted",
        )?;
        let mut bad = prior.clone();
        bad.push(0);
        super::need(
            payload(row, &bad, &current, "0644").is_err(),
            "T02 altered archive accepted",
        )?;
        let mut bad = row.clone();
        bad["edits"].as_array_mut().ok_or("T02 edits absent")?.pop();
        super::need(
            payload(&bad, &prior, &current, "0644").is_err(),
            "T02 missing edit accepted",
        )?;
    }
    for b in doc["callers"].as_array().ok_or("T02 callers absent")? {
        let hash = super::text(b, "current_sha256")?;
        let source = super::text(b, "source_path")?;
        let expected = super::text(b, "current_digest")?;
        let bytes = b["current_bytes"]
            .as_u64()
            .and_then(|n| usize::try_from(n).ok());
        caller(b, hash, source, expected, bytes)?;
        try_current(
            root,
            super::text(b, "container")?,
            hash,
            source,
            expected,
            bytes,
        )
        .ok_or("T02 current positive route absent")??;
        for (h, s, e, n) in [
            ("wrong", source, expected, bytes),
            (hash, "wrong", expected, bytes),
            (hash, source, "wrong", bytes),
            (
                hash,
                source,
                expected,
                if bytes.is_some() { None } else { Some(0) },
            ),
        ] {
            super::need(
                caller(b, h, s, e, n).is_err(),
                "T02 mutated caller accepted",
            )?;
        }
    }
    let mut changed = doc;
    changed["rows"][0]["current"]["sha256"] = json!("wrong");
    super::need(
        document(&serde_json::to_vec(&changed).map_err(|e| e.to_string())?).is_err(),
        "T02 rehashed sidecar accepted",
    )?;
    eprintln!(
        "T02_POST_A06_GENERATION_CONTROLS closed24_sources_77_callers_git_archives_modes_order_and_rehashed_mutations_rejected=true"
    );
    Ok(())
}

#[cfg(test)]
pub(super) fn control_operand(
    root: &Path,
    path: &str,
    expected: &Value,
    supplied_current: &[u8],
) -> Result<Vec<u8>, String> {
    super::need(expected["path"] == path, "T02 historical control path")?;
    let doc = load(root)?;
    super::need(
        [
            "crates/ssx-infra/src/search.rs",
            "crates/ssx-infra/src/search_deadline_drain_tests.rs",
            "crates/xtask/src/a09.rs",
        ]
        .contains(&path),
        "T02 unlisted historical control path",
    )?;
    if let Some(row) = find(&doc, expected) {
        let (prior, actual) = verify(root, row)?;
        super::need(
            supplied_current == actual,
            "T02 historical control supplied current differs",
        )?;
        return Ok(prior);
    }
    // The unchanged deadline-drain test retains its original exact current operand.
    super::need(
        path == "crates/ssx-infra/src/search_deadline_drain_tests.rs",
        "T02 unlisted historical control tuple",
    )?;
    let actual = super::read_exact(root, expected)?;
    super::need(
        supplied_current == actual,
        "T02 unchanged control supplied current differs",
    )?;
    Ok(actual)
}
