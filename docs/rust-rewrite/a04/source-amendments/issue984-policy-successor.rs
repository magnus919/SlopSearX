//! Exact source-custody bridge for the bounded Science date-scope policy edit.
//! Historical A13 and W06 qualification documents remain immutable.
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{fs, path::Path};

const MANIFEST: &str = "docs/rust-rewrite/a04/source-amendments/issue984-policy-successor-v1.json";
const MANIFEST_SHA: &str = "396d74b539a3265df066afbaf4420e2681343128dcd11324745a1e2aeb42c95c";
const SOURCE: &str = "crates/ssx-application/src/policy.rs";
const ARCHIVE: &str = "docs/rust-rewrite/a04/source-amendments/policy.rs.4008f0ec.frozen";
const OLD_SHA: &str = "5aa864100000ce0625540db4771cc5b8d2ce650beff1457b2a6b840277011681";
const OLD_BLOB: &str = "4008f0ecfc8694178702264fe08a7c1f73013f86";
const OLD_BYTES: usize = 38_237;
const CURRENT_SHA: &str = "f33db9d346b462a109fb566029de1d8bd620ef520f14795c232cfa3560b1d932";
const CURRENT_BYTES: usize = 38_727;

#[derive(Clone, Copy)]
struct Caller {
    path: &'static str,
    sha: &'static str,
    bytes: Option<usize>,
}

const CALLERS: [Caller; 4] = [
    Caller {
        path: "docs/rust-rewrite/a13/dehashed-source-map-v1.json",
        sha: "292c55e50d49bf53a0012d1c7a86ce422b2e58fe26f12bc2606b8f1621cdf18d",
        bytes: Some(OLD_BYTES),
    },
    Caller {
        path: "docs/rust-rewrite/a13/qualification-v1.json",
        sha: "77661b6d69dc2926a2d01d277e68e22932d7fb1aae258fb272ab1192dff18c6e",
        bytes: Some(OLD_BYTES),
    },
    Caller {
        path: "docs/rust-rewrite/a13/remaining/source-map.json",
        sha: "9f75190c18606f2e9416e4db9a99392c7cb9aa0836dc3ff41a56263f05af975f",
        bytes: Some(OLD_BYTES),
    },
    Caller {
        path: "docs/rust-rewrite/w06/qualification.json",
        sha: "2a03778871e35de5319b21db722c8a4679d6f0c8744d393f76ee6f89052ac7fb",
        bytes: None,
    },
];

fn require(ok: bool, message: &str) -> Result<(), String> {
    if ok { Ok(()) } else { Err(message.into()) }
}

fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn mode(path: &Path) -> Result<String, String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        Ok(format!(
            "{:04o}",
            fs::symlink_metadata(path)
                .map_err(|error| error.to_string())?
                .permissions()
                .mode()
                & 0o7777
        ))
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        Err("issue #984 policy source mode proof requires Unix".into())
    }
}

fn read_regular(root: &Path, path: &str, expected_mode: &str) -> Result<Vec<u8>, String> {
    let full = root.join(path);
    let metadata = fs::symlink_metadata(&full).map_err(|error| error.to_string())?;
    require(
        metadata.file_type().is_file() && !metadata.file_type().is_symlink(),
        "issue #984 proof input must be a regular non-symlink file",
    )?;
    require(
        mode(&full)? == expected_mode,
        "issue #984 proof input mode differs",
    )?;
    fs::read(full).map_err(|error| error.to_string())
}

fn load(root: &Path) -> Result<Value, String> {
    let raw = read_regular(root, MANIFEST, "0644")?;
    require(
        digest(&raw) == MANIFEST_SHA,
        "issue #984 policy successor manifest changed",
    )?;
    let document: Value = serde_json::from_slice(&raw).map_err(|error| error.to_string())?;
    validate_scope(&document)?;
    Ok(document)
}

fn validate_scope(document: &Value) -> Result<(), String> {
    require(
        document["schema"] == "A04_ISSUE984_POLICY_SUCCESSOR_V1"
            && document["source_path"] == SOURCE
            && document["predecessor"]["archive"] == ARCHIVE
            && document["predecessor"]["git_blob"] == OLD_BLOB
            && document["predecessor"]["sha256"] == OLD_SHA
            && document["predecessor"]["bytes"].as_u64() == Some(OLD_BYTES as u64)
            && document["predecessor"]["mode"] == "0644"
            && document["current"]["sha256"] == CURRENT_SHA
            && document["current"]["bytes"].as_u64() == Some(CURRENT_BYTES as u64)
            && document["current"]["mode"] == "0644",
        "issue #984 policy successor identity differs",
    )?;
    let rows = document["callers"]
        .as_array()
        .ok_or("issue #984 callers absent")?;
    require(
        rows.len() == CALLERS.len(),
        "issue #984 caller count differs",
    )?;
    for (row, caller) in rows.iter().zip(CALLERS) {
        require(
            row["container_path"] == caller.path
                && row["container_sha256"] == caller.sha
                && row["expected_sha256"] == OLD_SHA
                && row["expected_bytes"].as_u64().map(|n| n as usize) == caller.bytes,
            "issue #984 caller tuple differs",
        )?;
    }
    Ok(())
}

fn reconstruct(document: &Value, predecessor: &[u8]) -> Result<Vec<u8>, String> {
    require(
        digest(predecessor) == OLD_SHA && predecessor.len() == OLD_BYTES,
        "issue #984 predecessor bytes differ",
    )?;
    let edits = document["edits"]
        .as_array()
        .ok_or("issue #984 edit list absent")?;
    require(!edits.is_empty(), "issue #984 edit list empty")?;
    let mut result = Vec::new();
    let mut cursor = 0usize;
    let mut previous_offset = None;
    for edit in edits {
        let at = edit["at"]
            .as_u64()
            .and_then(|value| usize::try_from(value).ok())
            .ok_or("issue #984 edit offset invalid")?;
        let remove = edit["remove_utf8"]
            .as_str()
            .ok_or("issue #984 removed bytes absent")?
            .as_bytes();
        let insert = edit["insert_utf8"]
            .as_str()
            .ok_or("issue #984 inserted bytes absent")?
            .as_bytes();
        require(
            previous_offset.is_none_or(|previous| previous < at)
                && at >= cursor
                && at
                    .checked_add(remove.len())
                    .is_some_and(|end| end <= predecessor.len()),
            "issue #984 edits overlap, reorder or exceed predecessor",
        )?;
        let end = at + remove.len();
        require(
            predecessor.get(at..end) == Some(remove),
            "issue #984 reviewed edit preimage differs",
        )?;
        result.extend_from_slice(&predecessor[cursor..at]);
        result.extend_from_slice(insert);
        cursor = end;
        previous_offset = Some(at);
    }
    result.extend_from_slice(&predecessor[cursor..]);
    Ok(result)
}

fn validate_transition(
    document: &Value,
    predecessor: &[u8],
    current: &[u8],
    predecessor_mode: &str,
    current_mode: &str,
) -> Result<(), String> {
    validate_scope(document)?;
    require(
        predecessor_mode == "0644" && current_mode == "0644",
        "issue #984 source mode differs",
    )?;
    require(
        current.len() == CURRENT_BYTES && digest(current) == CURRENT_SHA,
        "issue #984 current source bytes differ",
    )?;
    require(
        reconstruct(document, predecessor)? == current,
        "issue #984 ordered patch does not reconstruct current source",
    )
}

fn check_caller_material(raw: &[u8], caller: Caller) -> Result<(), String> {
    let value: Value = serde_json::from_slice(raw).map_err(|error| error.to_string())?;
    if caller.path == "docs/rust-rewrite/w06/qualification.json" {
        return require(
            value["owned_sources"][SOURCE] == OLD_SHA,
            "issue #984 W06 policy caller pin differs",
        );
    }
    let rows = value["sources"]
        .as_array()
        .ok_or("issue #984 A13 caller sources absent")?;
    let selected: Vec<_> = rows.iter().filter(|row| row["path"] == SOURCE).collect();
    require(
        selected.len() == 1
            && selected[0]["sha256"] == OLD_SHA
            && selected[0]["bytes"].as_u64() == Some(OLD_BYTES as u64),
        "issue #984 A13 policy caller pin differs",
    )
}

pub(crate) fn try_verify(
    root: &Path,
    container_path: &str,
    container_sha: &str,
    source_path: &str,
    expected_sha: &str,
    expected_bytes: Option<usize>,
) -> Option<Result<(), String>> {
    if source_path != SOURCE {
        return None;
    }
    let caller = CALLERS
        .iter()
        .copied()
        .find(|caller| caller.path == container_path)?;
    Some((|| {
        require(
            container_sha == caller.sha
                && expected_sha == OLD_SHA
                && expected_bytes == caller.bytes,
            "issue #984 exact policy caller tuple differs",
        )?;
        let document = load(root)?;
        let caller_bytes =
            super::a06_source_successor::historical_caller(root, container_path, container_sha)?;
        check_caller_material(&caller_bytes, caller)?;
        let predecessor = read_regular(root, ARCHIVE, "0644")?;
        let current = read_regular(root, SOURCE, "0644")?;
        validate_transition(&document, &predecessor, &current, "0644", "0644")
    })())
}

#[cfg(test)]
pub(crate) fn controls(root: &Path) -> Result<(), String> {
    let document = load(root)?;
    let predecessor = read_regular(root, ARCHIVE, "0644")?;
    let current = read_regular(root, SOURCE, "0644")?;
    validate_transition(&document, &predecessor, &current, "0644", "0644")?;
    for caller in CALLERS {
        let result = try_verify(root, caller.path, caller.sha, SOURCE, OLD_SHA, caller.bytes)
            .ok_or("issue #984 positive caller route absent")?;
        result?;
        for (bad_container_sha, bad_source_sha, bad_bytes) in [
            ("wrong-container", OLD_SHA, caller.bytes),
            (caller.sha, "wrong-source", caller.bytes),
            (caller.sha, OLD_SHA, Some(0)),
        ] {
            require(
                try_verify(
                    root,
                    caller.path,
                    bad_container_sha,
                    SOURCE,
                    bad_source_sha,
                    bad_bytes,
                )
                .is_some_and(|result| result.is_err()),
                "issue #984 altered caller tuple accepted",
            )?;
        }
    }
    require(
        try_verify(root, "unlisted.json", "wrong", SOURCE, OLD_SHA, None).is_none()
            && try_verify(
                root,
                CALLERS[0].path,
                CALLERS[0].sha,
                "unlisted.rs",
                OLD_SHA,
                None,
            )
            .is_none(),
        "issue #984 unlisted context intercepted",
    )?;

    let mut changed = current.clone();
    changed.push(b' ');
    require(
        validate_transition(&document, &predecessor, &changed, "0644", "0644").is_err(),
        "issue #984 changed current source accepted",
    )?;
    let mut changed_predecessor = predecessor.clone();
    changed_predecessor.push(b' ');
    require(
        validate_transition(&document, &changed_predecessor, &current, "0644", "0644").is_err(),
        "issue #984 changed predecessor accepted",
    )?;
    for (old_mode, new_mode) in [("0755", "0644"), ("0644", "0600"), ("4644", "0644")] {
        require(
            validate_transition(&document, &predecessor, &current, old_mode, new_mode).is_err(),
            "issue #984 altered mode accepted",
        )?;
    }
    for mutation in ["remove", "duplicate", "reorder", "offset", "preimage"] {
        let mut bad = document.clone();
        let edits = bad["edits"]
            .as_array_mut()
            .ok_or("issue #984 edits absent")?;
        match mutation {
            "remove" => {
                edits.pop();
            }
            "duplicate" => {
                let first = edits[0].clone();
                edits.push(first);
            }
            "reorder" => edits.reverse(),
            "offset" => edits[0]["at"] = serde_json::json!(OLD_BYTES + 1),
            "preimage" => edits[0]["insert_utf8"] = serde_json::json!("unreviewed"),
            _ => unreachable!(),
        }
        require(
            validate_transition(&bad, &predecessor, &current, "0644", "0644").is_err(),
            "issue #984 mutated ordered patch accepted",
        )?;
    }
    let mut bad_manifest = document.clone();
    bad_manifest["callers"][0]["container_sha256"] = serde_json::json!("wrong");
    require(
        validate_scope(&bad_manifest).is_err(),
        "issue #984 altered caller manifest accepted",
    )?;
    physical_input_controls(root)?;
    let fallback_caller = CALLERS[3];
    let fallback_source = "crates/ssx-application/src/staged.rs";
    let fallback_sha = "c153cb0f71c4ac661e7dcb26b429ad66285c4c3debba8178711bf8f7ae537a75";
    require(
        try_verify(
            root,
            fallback_caller.path,
            fallback_caller.sha,
            fallback_source,
            fallback_sha,
            None,
        )
        .is_none()
            && super::a06_source_successor::try_verify(
                root,
                fallback_caller.path,
                fallback_caller.sha,
                fallback_source,
                fallback_sha,
                None,
            )
            .is_none(),
        "issue #984 unrelated W06 route intercepted before legacy fallback",
    )?;
    let legacy = super::super::verify(
        root,
        fallback_caller.path,
        fallback_caller.sha,
        fallback_source,
        fallback_sha,
        None,
    );
    require(
        legacy.is_ok()
            && super::verify_current(
                root,
                fallback_caller.path,
                fallback_caller.sha,
                fallback_source,
                fallback_sha,
                None,
            ) == legacy,
        "issue #984 unrelated valid W06 route changed legacy verifier result",
    )?;
    let invalid_legacy = super::super::verify(
        root,
        fallback_caller.path,
        fallback_caller.sha,
        fallback_source,
        "wrong",
        None,
    );
    require(
        invalid_legacy.is_err()
            && super::verify_current(
                root,
                fallback_caller.path,
                fallback_caller.sha,
                fallback_source,
                "wrong",
                None,
            ) == invalid_legacy,
        "issue #984 unrelated invalid W06 route changed legacy verifier result",
    )?;
    eprintln!(
        "ISSUE984_POLICY_SUCCESSOR_CONTROLS four_callers=true exact_current_and_history=true altered_context_modes_archive_patch_manifest_physical_inputs_and_fallback_rejected=true"
    );
    Ok(())
}

#[cfg(all(test, unix))]
fn physical_input_controls(root: &Path) -> Result<(), String> {
    use std::os::unix::fs::{PermissionsExt, symlink};
    struct Owned(std::path::PathBuf);
    impl Drop for Owned {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn fixture(root: &Path) -> Result<Owned, String> {
        const GENERATION: &str =
            "docs/rust-rewrite/a06/source-amendments/post-a06-generation-v1.json";
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|error| error.to_string())?
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "issue984-policy-controls-{}-{suffix}",
            std::process::id()
        ));
        fs::create_dir(&directory).map_err(|error| error.to_string())?;
        let owned = Owned(directory);
        for path in [
            MANIFEST,
            ARCHIVE,
            SOURCE,
            GENERATION,
            CALLERS[0].path,
            CALLERS[1].path,
            CALLERS[2].path,
            CALLERS[3].path,
        ] {
            let source = root.join(path);
            let destination = owned.0.join(path);
            fs::create_dir_all(destination.parent().ok_or("issue #984 fixture parent")?)
                .map_err(|error| error.to_string())?;
            fs::copy(source, &destination).map_err(|error| error.to_string())?;
            fs::set_permissions(&destination, fs::Permissions::from_mode(0o644))
                .map_err(|error| error.to_string())?;
        }
        let generation: Value = serde_json::from_slice(
            &fs::read(owned.0.join(GENERATION)).map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
        let rows = generation["rows"]
            .as_array()
            .ok_or("issue #984 fixture A06 generation rows absent")?;
        for caller in CALLERS {
            require(
                !rows.iter().any(|row| {
                    row["previous"]["path"] == caller.path
                        && row["previous"]["sha256"] == caller.sha
                }),
                "issue #984 fixture unexpectedly depends on an A06 generation row",
            )?;
        }
        Ok(owned)
    }
    fn positive(root: &Path) -> Result<(), String> {
        for caller in CALLERS {
            try_verify(root, caller.path, caller.sha, SOURCE, OLD_SHA, caller.bytes)
                .ok_or("issue #984 fixture caller route absent")??;
        }
        Ok(())
    }
    fn rejected(root: &Path, caller: Caller) -> Result<(), String> {
        require(
            try_verify(root, caller.path, caller.sha, SOURCE, OLD_SHA, caller.bytes)
                .is_some_and(|result| result.is_err()),
            "issue #984 mutated physical proof input accepted",
        )
    }
    fn rejected_all(root: &Path) -> Result<(), String> {
        for caller in CALLERS {
            rejected(root, caller)?;
        }
        Ok(())
    }
    fn restore(root: &Path, path: &str, bytes: &[u8]) -> Result<(), String> {
        let full = root.join(path);
        fs::write(&full, bytes).map_err(|error| error.to_string())?;
        fs::set_permissions(full, fs::Permissions::from_mode(0o644))
            .map_err(|error| error.to_string())
    }

    let fixture = fixture(root)?;
    let root = fixture.0.as_path();
    positive(root)?;

    let manifest = root.join(MANIFEST);
    let original_manifest = fs::read(&manifest).map_err(|error| error.to_string())?;
    let mut bytes = original_manifest.clone();
    bytes.push(b' ');
    fs::write(&manifest, bytes).map_err(|error| error.to_string())?;
    rejected_all(root)?;
    restore(root, MANIFEST, &original_manifest)?;
    positive(root)?;

    for caller in CALLERS {
        let caller_path = root.join(caller.path);
        let original = fs::read(&caller_path).map_err(|error| error.to_string())?;
        let mut changed = original.clone();
        changed.push(b' ');
        fs::write(&caller_path, changed).map_err(|error| error.to_string())?;
        rejected(root, caller)?;
        restore(root, caller.path, &original)?;
        positive(root)?;
    }

    let archive = root.join(ARCHIVE);
    let original_archive = fs::read(&archive).map_err(|error| error.to_string())?;
    let mut bytes = original_archive.clone();
    bytes.push(b' ');
    fs::write(&archive, bytes).map_err(|error| error.to_string())?;
    rejected_all(root)?;
    restore(root, ARCHIVE, &original_archive)?;
    positive(root)?;

    let current = root.join(SOURCE);
    let original_current = fs::read(&current).map_err(|error| error.to_string())?;
    fs::write(&current, &original_archive).map_err(|error| error.to_string())?;
    rejected_all(root)?;
    restore(root, SOURCE, &original_current)?;
    positive(root)?;

    for (path, selected_caller) in [
        (MANIFEST, None),
        (ARCHIVE, None),
        (SOURCE, None),
        (CALLERS[0].path, Some(CALLERS[0])),
        (CALLERS[1].path, Some(CALLERS[1])),
        (CALLERS[2].path, Some(CALLERS[2])),
        (CALLERS[3].path, Some(CALLERS[3])),
    ] {
        let original = fs::read(root.join(path)).map_err(|error| error.to_string())?;
        let link = root.join(path);
        let target = root.join(format!("issue984-symlink-target-{}", std::process::id()));
        fs::write(&target, &original).map_err(|error| error.to_string())?;
        fs::set_permissions(&target, fs::Permissions::from_mode(0o644))
            .map_err(|error| error.to_string())?;
        fs::remove_file(&link).map_err(|error| error.to_string())?;
        symlink(&target, &link).map_err(|error| error.to_string())?;
        require(
            fs::read(&link).map_err(|error| error.to_string())? == original,
            "issue #984 symlink control target changed file bytes",
        )?;
        if let Some(caller) = selected_caller {
            rejected(root, caller)?;
        } else {
            rejected_all(root)?;
        }
        fs::remove_file(&link).map_err(|error| error.to_string())?;
        fs::remove_file(&target).map_err(|error| error.to_string())?;
        restore(root, path, &original)?;
        positive(root)?;
    }
    Ok(())
}

#[cfg(all(test, not(unix)))]
fn physical_input_controls(_: &Path) -> Result<(), String> {
    Err("issue #984 physical file controls require Unix modes and symlinks".into())
}
