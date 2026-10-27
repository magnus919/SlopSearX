//! Closed native config/registry successors of frozen adapter caller rows.
//! Actual source is proven by exact inverse and forward edits; no old manifest
//! is rewritten, and no unlisted caller gains source-drift acceptance.
use serde_json::Value;
#[cfg(test)]
use serde_json::json;
use std::{fs, path::Path};
const CAPSULE: &str = "docs/rust-rewrite/native-engines/catalog-source-amendment.json";
const CAPSULE_SHA: &str = "841c929ea11f04c777275ed7aed8453e2782f23f9a457622b53d0e920825de69";
const SOURCES: &[&str] = &[
    "crates/ssx-config/src/lib.rs",
    "crates/ssx-registry/src/lib.rs",
];
const CONTAINERS: &[&str] = &[
    "docs/rust-rewrite/a10/qualification.json",
    "docs/rust-rewrite/a11/qualification.json",
    "docs/rust-rewrite/a13/dehashed-source-map-v1.json",
    "docs/rust-rewrite/a13/qualification-v1.json",
    "docs/rust-rewrite/a13/remaining/source-map.json",
];
fn regular(root: &Path, relative: &str) -> Result<(Vec<u8>, String), String> {
    let path = root.join(relative);
    let metadata = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
    super::need(
        metadata.is_file() && !metadata.file_type().is_symlink(),
        "native catalog source must be regular, not symlink",
    )?;
    Ok((
        fs::read(&path).map_err(|e| e.to_string())?,
        super::permissions(&path)?,
    ))
}
fn document(raw: &[u8]) -> Result<Value, String> {
    super::need(
        super::digest(raw) == CAPSULE_SHA,
        "native catalog capsule identity differs",
    )?;
    let doc: Value = serde_json::from_slice(raw).map_err(|e| e.to_string())?;
    super::need(
        doc["schema"] == "DEEPSEEK_NATIVE_CATALOG_SOURCE_SUCCESSOR_V1"
            && doc["baseline_commit"] == "e12744511013bd05370173e777daaef471bf9527"
            && doc["sources"]
                .as_array()
                .is_some_and(|rows| rows.len() == 2)
            && doc["bindings"]
                .as_array()
                .is_some_and(|rows| rows.len() == 13),
        "native catalog capsule closed scope differs",
    )?;
    Ok(doc)
}
fn load(root: &Path) -> Result<Value, String> {
    let (raw, mode) = regular(root, CAPSULE)?;
    super::need(mode == "0644", "native catalog capsule mode differs")?;
    document(&raw)
}
fn inverse(row: &Value, current: &[u8], mode: &str) -> Result<Vec<u8>, String> {
    super::need(
        row["prior"]["path"] == row["current"]["path"]
            && SOURCES.contains(&super::text(&row["prior"], "path")?),
        "native catalog source scope differs",
    )?;
    super::material(&row["current"], current, mode)?;
    super::need(
        super::digest(&serde_json::to_vec(&row["edits"]).map_err(|e| e.to_string())?)
            == super::text(row, "edits_sha256")?,
        "native catalog reviewed edit digest differs",
    )?;
    let mut prior = Vec::new();
    let (mut before_cursor, mut current_cursor) = (0usize, 0usize);
    for edit in row["edits"]
        .as_array()
        .ok_or("native catalog edits absent")?
    {
        super::need(
            edit.as_object().is_some_and(|object| object.len() == 3),
            "native catalog edit shape differs",
        )?;
        let at = edit["at"]
            .as_u64()
            .and_then(|n| usize::try_from(n).ok())
            .ok_or("native catalog edit offset absent")?;
        let remove = super::text(edit, "remove_utf8")?.as_bytes();
        let insert = super::text(edit, "insert_utf8")?.as_bytes();
        let unchanged = at
            .checked_sub(before_cursor)
            .ok_or("native catalog edit order differs")?;
        let prefix_end = current_cursor
            .checked_add(unchanged)
            .ok_or("native catalog edit prefix overflow")?;
        let insert_end = prefix_end
            .checked_add(insert.len())
            .ok_or("native catalog insertion overflow")?;
        super::need(
            !remove.is_empty() || !insert.is_empty(),
            "native catalog empty edit",
        )?;
        prior.extend_from_slice(
            current
                .get(current_cursor..prefix_end)
                .ok_or("native catalog unchanged prefix absent")?,
        );
        super::need(
            current.get(prefix_end..insert_end) == Some(insert),
            "native catalog exact insertion differs",
        )?;
        prior.extend_from_slice(remove);
        before_cursor = at
            .checked_add(remove.len())
            .ok_or("native catalog removal overflow")?;
        current_cursor = insert_end;
    }
    prior.extend_from_slice(
        current
            .get(current_cursor..)
            .ok_or("native catalog trailing source absent")?,
    );
    super::material(&row["prior"], &prior, "0644")?;
    super::need(
        super::edits(&prior, &row["edits"])? == current,
        "native catalog forward reconstruction differs",
    )?;
    Ok(prior)
}
fn selected(container: &str, source: &str) -> bool {
    (CONTAINERS.contains(&container) && SOURCES.contains(&source))
        || (container == "docs/rust-rewrite/a12/qualification-v1.json"
            && source == "crates/ssx-config/src/lib.rs")
}
fn binding<'a>(
    doc: &'a Value,
    container: &str,
    container_sha: &str,
    source: &str,
    expected: &str,
    bytes: Option<usize>,
) -> Result<&'a Value, String> {
    let rows: Vec<_> = doc["bindings"]
        .as_array()
        .ok_or("native catalog bindings absent")?
        .iter()
        .filter(|row| {
            row["container"]["path"] == container
                && row["container"]["sha256"] == container_sha
                && row["source_row"]["path"] == source
        })
        .collect();
    super::need(
        rows.len() == 1,
        "native catalog exact caller absent or duplicated",
    )?;
    let row = rows[0];
    super::need(
        row["source_row"]["sha256"] == expected
            && row["source_row"]["bytes"]
                .as_u64()
                .and_then(|n| usize::try_from(n).ok())
                == bytes,
        "native catalog exact caller source tuple differs",
    )?;
    Ok(row)
}
fn verify(root: &Path, doc: &Value, binding: &Value) -> Result<(), String> {
    let container_path = super::text(&binding["container"], "path")?;
    let (_, container_mode) = regular(root, container_path)?;
    super::need(
        container_mode == "0644",
        "native catalog caller physical mode differs",
    )?;
    let original = super::historical_caller(
        root,
        container_path,
        super::text(&binding["container"], "sha256")?,
    )?;
    super::material(&binding["container"], &original, "0644")?;
    let container: Value = serde_json::from_slice(&original).map_err(|e| e.to_string())?;
    super::need(
        container.pointer(super::text(binding, "pointer")?) == Some(&binding["source_row"]),
        "native catalog frozen caller row differs",
    )?;
    let mut selected_source = false;
    for row in doc["sources"]
        .as_array()
        .ok_or("native catalog source frame absent")?
    {
        let path = super::text(&row["current"], "path")?;
        let (current, mode) = regular(root, path)?;
        let prior = inverse(row, &current, &mode)?;
        if row["prior"]["path"] == binding["source_row"]["path"] {
            super::need(
                row["prior"]["sha256"] == binding["source_row"]["sha256"]
                    && binding["source_row"]
                        .get("bytes")
                        .is_none_or(|bytes| row["prior"]["bytes"] == *bytes)
                    && super::digest(&prior) == super::text(&binding["source_row"], "sha256")?,
                "native catalog source-to-caller predecessor differs",
            )?;
            selected_source = true;
        }
    }
    super::need(
        selected_source,
        "native catalog caller source not in physical frame",
    )
}
pub(super) fn try_verify(
    root: &Path,
    container: &str,
    container_sha: &str,
    source: &str,
    expected: &str,
    bytes: Option<usize>,
) -> Option<Result<(), String>> {
    if !selected(container, source) {
        return None;
    }
    Some((|| {
        let doc = load(root)?;
        let row = binding(&doc, container, container_sha, source, expected, bytes)?;
        verify(root, &doc, row)
    })())
}
#[cfg(test)]
pub(super) fn controls(root: &Path) -> Result<(), String> {
    let doc = load(root)?;
    for row in doc["bindings"]
        .as_array()
        .ok_or("native catalog control bindings absent")?
    {
        let container = super::text(&row["container"], "path")?;
        let hash = super::text(&row["container"], "sha256")?;
        let source = super::text(&row["source_row"], "path")?;
        let expected = super::text(&row["source_row"], "sha256")?;
        let bytes = row["source_row"]["bytes"].as_u64().map(|n| n as usize);
        // Use the same entrypoint called by the complete original A13 source
        // inventories, not only the amendment's private payload checker.
        super::super::verify_current(root, container, hash, source, expected, bytes)?;
        for (bad_hash, bad_expected, bad_bytes) in [
            ("wrong", expected, bytes),
            (hash, "wrong", bytes),
            (hash, expected, if bytes.is_some() { None } else { Some(0) }),
            (hash, expected, Some(bytes.unwrap_or(0) + 1)),
        ] {
            super::need(
                super::super::verify_current(
                    root,
                    container,
                    bad_hash,
                    source,
                    bad_expected,
                    bad_bytes,
                )
                .is_err(),
                "native catalog actual A13 entrypoint accepted altered caller tuple",
            )?;
        }
        if bytes.is_none() {
            let prior_length = doc["sources"]
                .as_array()
                .unwrap()
                .iter()
                .find(|source_row| source_row["prior"]["path"] == source)
                .unwrap()["prior"]["bytes"]
                .as_u64()
                .unwrap() as usize;
            super::need(
                super::super::verify_current(
                    root,
                    container,
                    hash,
                    source,
                    expected,
                    Some(prior_length),
                )
                .is_err(),
                "native catalog hash-only caller accepted an invented length",
            )?;
        }
        let mut wrong = row.clone();
        wrong["pointer"] = json!("/sources/0");
        super::need(
            verify(root, &doc, &wrong).is_err(),
            "native catalog moved frozen caller accepted",
        )?;
        wrong = row.clone();
        wrong["source_row"]["extra"] = json!(true);
        super::need(
            verify(root, &doc, &wrong).is_err(),
            "native catalog changed frozen caller row accepted",
        )?;
    }
    for row in doc["sources"].as_array().unwrap() {
        let (current, mode) = regular(root, super::text(&row["current"], "path")?)?;
        let prior = inverse(row, &current, &mode)?;
        super::need(
            inverse(row, &prior, "0644").is_err(),
            "native catalog historical bytes accepted as current",
        )?;
        for mode in ["0600", "0755", "4644"] {
            super::need(
                inverse(row, &current, mode).is_err(),
                "native catalog source mode mutant accepted",
            )?;
        }
        let mut altered = current.clone();
        altered.push(b'\n');
        let mut rehashed = row.clone();
        rehashed["current"]["sha256"] = json!(super::digest(&altered));
        rehashed["current"]["bytes"] = json!(altered.len());
        super::need(
            inverse(&rehashed, &altered, "0644").is_err(),
            "native catalog rehashed unrelated source accepted",
        )?;
        for mutation in 0..5 {
            let mut wrong = row.clone();
            let edits = wrong["edits"].as_array_mut().unwrap();
            match mutation {
                0 => {
                    edits.pop();
                }
                1 => {
                    edits.insert(0, edits[0].clone());
                }
                2 => {
                    edits[0]["at"] = json!(0);
                }
                3 => {
                    edits[0]["insert_utf8"] = json!("unrelated edit");
                }
                _ => {
                    edits[0]["extra"] = json!(true);
                }
            }
            // Recompute the proposed edit digest so structural guards, not
            // merely the capsule's seal, must reject the changed edit stream.
            wrong["edits_sha256"] = json!(super::digest(
                &serde_json::to_vec(&wrong["edits"]).map_err(|e| e.to_string())?
            ));
            super::need(
                inverse(&wrong, &current, "0644").is_err(),
                "native catalog missing/duplicate/moved/changed/extra edit accepted",
            )?;
        }
    }
    let mut changed = fs::read(root.join(CAPSULE)).map_err(|e| e.to_string())?;
    changed.push(b'\n');
    super::need(
        document(&changed).is_err(),
        "native catalog altered capsule accepted",
    )?;
    super::need(
        try_verify(root, "unlisted.json", "wrong", SOURCES[0], "wrong", None).is_none(),
        "native catalog changed unlisted container dispatch",
    )?;
    super::need(
        try_verify(
            root,
            "docs/rust-rewrite/a12/qualification-v1.json",
            "wrong",
            SOURCES[1],
            "wrong",
            None,
        )
        .is_none(),
        "native catalog changed unlisted A12 registry dispatch",
    )?;
    super::need(
        try_verify(root, CONTAINERS[0], "wrong", "unlisted.rs", "wrong", None).is_none(),
        "native catalog changed unlisted source dispatch",
    )?;
    super::need(
        try_verify(
            Path::new("/missing-native-catalog-root"),
            CONTAINERS[0],
            "wrong",
            SOURCES[0],
            "wrong",
            None,
        )
        .is_some_and(|r| r.is_err()),
        "native catalog missing physical frame accepted",
    )?;
    Ok(())
}
