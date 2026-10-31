//! Issue1077: exact physical current sources precede two historical operands.
//! This local admission leaves the frozen W08 qualification and shared readers intact.
use super::digest;
use crate::Result;
use std::{fs, path::Path, process::Command};
const BASE: &str = "d9d42277657394e9f62061072a95671345a94f38";
const SELF_PATH: &str = "crates/xtask/src/w08/snapshot_selection_successor.rs";
const COMPILED_SELF: &[u8] = include_bytes!("snapshot_selection_successor.rs");
struct Edit {
    offset: usize,
    current: &'static [u8],
    prior: &'static [u8],
}
struct Row {
    path: &'static str,
    prior_bytes: usize,
    prior_sha: &'static str,
    prior_blob: &'static str,
    current_bytes: usize,
    current_sha: &'static str,
    edits: &'static [Edit],
}
const ROWS: &[Row; 2] = &[
Row { path: "crates/ssx-application/src/composition.rs", prior_bytes: 23541, prior_sha: "50270b17f692104c481f19475c055584ef42af36c9864774347d5d27e9852e3f", prior_blob: "642ef59cd096132f683390c070601b87ca7fd3a1", current_bytes: 23605, current_sha: "213547e1ebeb0bdf1bb31f6b9669c0c3dec5775db42399207ca60ec04964d38d", edits: &[
Edit { offset: 9112, current: b"        count: usize,\n", prior: b"" },
Edit { offset: 9314, current: b"", prior: b"        let count = indices.len();\n" },
Edit { offset: 10224, current: b"            (0..count.min(MAX_SOURCE_RESULTS)).collect(),\n            count,\n", prior: b"            (0..count).collect(),\n" },
Edit { offset: 11143, current: b"            1,\n", prior: b"" },
Edit { offset: 12107, current: b"            count,\n", prior: b"" }
] },
Row { path: "crates/ssx-application/tests/w08.rs", prior_bytes: 35830, prior_sha: "4ae215d754b138da6ed4677bdcb5702249605624fbac78f2a0c141c27e87c702", prior_blob: "32e99f09c3bf228b9ca5f6a4ca692f151b7ef8e2", current_bytes: 38887, current_sha: "791f14815c6197841e2db1bef0be60bb23a9a7fc46ea60992137c5fd985c0332", edits: &[
Edit { offset: 4705, current: b"        }]\n    }\n}\n// Explicit groups remain authoritative even beyond the displayed prefix.\nstruct ExplicitGroups(Vec<usize>);\nimpl Entities for ExplicitGroups {\n    fn groups(&self, _: &SearchSnapshot, _: u8) -> Vec<EntityGroup> {\n        vec![EntityGroup {\n            id: \"entity-1\".into(),\n            namespace: \"fixture\".into(),\n            result_indices: self.0.clone(),\n", prior: b"" },
Edit { offset: 16782, current: b"\n    // A large retained source still exposes only the ordered first 25 IDs,\n    // while its complete selection count controls status and metadata.\n    let first_25 = (0..25)\n        .map(|index| format!(\"snapshot-a:{index}\"))\n        .collect::<Vec<_>>();\n    for count in [0, 25, 26, 4096] {\n        let (resolver, store, _) =\n            self::resolver(Some(snapshot(count, 200.)), None, false, records());\n        let requested = artifact(ArtifactKind::Snapshot, \"snapshot-a\");\n        let source = run(resolver.resolve(\n            &tenant(),\n            &requested,\n            Destination::DependencyDossier,\n            &cancel,\n        ))\n        .unwrap();\n        assert_eq!(source.artifact, requested);\n        assert_eq!(source.query.as_deref(), Some(\"retained query\"));\n        assert_eq!(source.details[\"query_id\"], \"query-a\");\n        assert_eq!(source.details[\"result_count\"], count);\n        assert_eq!(source.details[\"included_result_count\"], count.min(25));\n        assert_eq!(\n            source.status,\n            if count > 25 {\n                SourceStatus::Truncated\n            } else {\n                SourceStatus::Live\n            }\n        );\n        assert_eq!(\n            source.result_ids,\n            if count == 0 { vec![] } else { first_25.clone() }\n        );\n        assert_eq!(store.reads.load(Ordering::SeqCst), 1);\n        assert_eq!(store.snapshot.as_ref().unwrap().results().len(), count);\n    }\n\n    let (mut explicit, _, _) = self::resolver(Some(snapshot(26, 200.)), None, false, records());\n    explicit.entities = Arc::new(ExplicitGroups((0..26).rev().collect()));\n    let requested = artifact(ArtifactKind::EntityGroup, &group);\n    let source = run(explicit.resolve(\n        &tenant(),\n        &requested,\n        Destination::DependencyDossier,\n        &cancel,\n    ))\n    .unwrap();\n    assert_eq!(source.artifact, requested);\n    assert_eq!(source.status, SourceStatus::Truncated);\n    assert_eq!(\n        source.result_ids,\n        (1..26)\n            .rev()\n            .map(|index| format!(\"snapshot-a:{index}\"))\n            .collect::<Vec<_>>()\n    );\n    assert_eq!(source.details[\"member_count\"], 26);\n    assert_eq!(source.details[\"included_result_count\"], 25);\n\n    // The first 25 members are valid: only complete validation can reject\n    // the out-of-range final member, which would not be displayed.\n    explicit.entities = Arc::new(ExplicitGroups((0..27).collect()));\n    assert_eq!(\n        run(explicit.resolve(\n            &tenant(),\n            &requested,\n            Destination::DependencyDossier,\n            &cancel,\n        ))\n        .unwrap_err()\n        .code,\n        \"source_malformed\"\n    );\n", prior: b"" }
] }
];
fn need(ok: bool, message: &str) -> Result<()> {
    if ok { Ok(()) } else { Err(message.into()) }
}
fn git(root: &Path, args: &[&str]) -> Result<Vec<u8>> {
    let output = Command::new("git")
        .arg("--no-replace-objects")
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|_| "snapshot selection Git evidence unavailable".to_owned())?;
    need(
        output.status.success(),
        "snapshot selection Git command failed",
    )?;
    Ok(output.stdout)
}
fn physical(root: &Path, path: &str) -> Result<(Vec<u8>, u32)> {
    let full = root.join(path);
    let metadata =
        fs::symlink_metadata(&full).map_err(|_| "snapshot selection source missing".to_owned())?;
    need(
        metadata.is_file() && !metadata.file_type().is_symlink(),
        "snapshot selection source nonregular",
    )?;
    let canonical_root = root
        .canonicalize()
        .map_err(|_| "snapshot selection root unavailable".to_owned())?;
    let canonical = full
        .canonicalize()
        .map_err(|_| "snapshot selection source unavailable".to_owned())?;
    need(
        canonical.starts_with(canonical_root),
        "snapshot selection source escapes root",
    )?;
    #[cfg(unix)]
    let mode = {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o7777
    };
    #[cfg(windows)]
    let mode = {
        let indexed = git(root, &["ls-files", "--stage", "--", path])?;
        let indexed = std::str::from_utf8(&indexed)
            .map_err(|_| "snapshot selection index encoding".to_owned())?;
        need(
            indexed.starts_with("100644 ")
                && indexed.lines().count() == 1
                && indexed.ends_with(&format!("\t{path}\n")),
            "snapshot selection source mode",
        )?;
        0o644
    };
    let raw = fs::read(full).map_err(|_| "snapshot selection source unreadable".to_owned())?;
    Ok((raw, mode))
}
fn authenticated_module(root: &Path) -> Result<()> {
    let (raw, mode) = physical(root, SELF_PATH)?;
    need(
        mode == 0o644 && raw == COMPILED_SELF,
        "snapshot selection compiled admission differs",
    )
}
fn historical(root: &Path, row: &Row) -> Result<Vec<u8>> {
    let tree = git(root, &["ls-tree", "-z", BASE, "--", row.path])?;
    need(
        tree == format!("100644 blob {}\t{}\0", row.prior_blob, row.path).as_bytes(),
        "snapshot selection prior Git identity",
    )?;
    let raw = git(root, &["cat-file", "blob", row.prior_blob])?;
    need(
        raw.len() == row.prior_bytes && digest(&raw) == row.prior_sha,
        "snapshot selection prior Git bytes",
    )?;
    Ok(raw)
}
fn inverse(row: &Row, supplied: &[u8], mode: u32) -> Result<Vec<u8>> {
    need(mode == 0o644, "snapshot selection source mode")?;
    need(
        supplied.len() == row.current_bytes && digest(supplied) == row.current_sha,
        "snapshot selection current tuple",
    )?;
    let mut prior = supplied.to_vec();
    for edit in row.edits.iter().rev() {
        let end = edit
            .offset
            .checked_add(edit.current.len())
            .ok_or("snapshot selection span overflow")?;
        need(
            prior.get(edit.offset..end) == Some(edit.current),
            "snapshot selection exact inverse span",
        )?;
        prior.splice(edit.offset..end, edit.prior.iter().copied());
    }
    need(
        prior.len() == row.prior_bytes && digest(&prior) == row.prior_sha,
        "snapshot selection exact prior tuple",
    )?;
    Ok(prior)
}
/// Return historical logical bytes only after authenticating the real current file.
pub(super) fn prior(root: &Path, path: &str, expected: &str, supplied: &[u8]) -> Result<Vec<u8>> {
    let row = ROWS
        .iter()
        .find(|row| row.path == path && row.prior_sha == expected)
        .ok_or("unlisted snapshot selection caller")?;
    authenticated_module(root)?;
    let (actual, mode) = physical(root, path)?;
    need(
        actual == supplied,
        "snapshot selection supplied bytes differ from physical source",
    )?;
    let prior = inverse(row, supplied, mode)?;
    need(
        prior == historical(root, row)?,
        "snapshot selection inverse differs from Git prior",
    )?;
    Ok(prior)
}
#[cfg(unix)]
fn filesystem_controls(row: &Row, current: &[u8], old: &[u8]) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    use std::{
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };
    struct Owned(PathBuf, bool);
    impl Drop for Owned {
        fn drop(&mut self) {
            if !self.1 {
                let _ = fs::remove_dir_all(&self.0);
            }
        }
    }
    let serial = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "snapshot selection fixture clock".to_owned())?
        .as_nanos();
    let path = std::env::temp_dir().join(format!("ssx-1077-{}-{serial}", std::process::id()));
    fs::create_dir(&path).map_err(|_| "snapshot selection fixture create".to_owned())?;
    let mut owned = Owned(path, false);
    let module = owned.0.join(SELF_PATH);
    fs::create_dir_all(
        module
            .parent()
            .ok_or("snapshot selection fixture module path")?,
    )
    .map_err(|_| "snapshot selection fixture directory".to_owned())?;
    fs::write(&module, COMPILED_SELF)
        .map_err(|_| "snapshot selection fixture module write".to_owned())?;
    fs::set_permissions(&module, fs::Permissions::from_mode(0o644))
        .map_err(|_| "snapshot selection fixture mode".to_owned())?;
    let target = owned.0.join(row.path);
    fs::create_dir_all(
        target
            .parent()
            .ok_or("snapshot selection fixture source path")?,
    )
    .map_err(|_| "snapshot selection fixture directory".to_owned())?;
    need(
        prior(&owned.0, row.path, row.prior_sha, current)
            .err()
            .as_deref()
            == Some("snapshot selection source missing"),
        "snapshot selection missing-file control",
    )?;
    fs::write(&target, old).map_err(|_| "snapshot selection fixture prior write".to_owned())?;
    fs::set_permissions(&target, fs::Permissions::from_mode(0o644))
        .map_err(|_| "snapshot selection fixture mode".to_owned())?;
    need(
        prior(&owned.0, row.path, row.prior_sha, old)
            .err()
            .as_deref()
            == Some("snapshot selection current tuple"),
        "snapshot selection old-as-current control",
    )?;
    fs::write(&target, current)
        .map_err(|_| "snapshot selection fixture current write".to_owned())?;
    fs::set_permissions(&target, fs::Permissions::from_mode(0o600))
        .map_err(|_| "snapshot selection fixture mode".to_owned())?;
    need(
        prior(&owned.0, row.path, row.prior_sha, current)
            .err()
            .as_deref()
            == Some("snapshot selection source mode"),
        "snapshot selection wrong-mode control",
    )?;
    fs::remove_dir_all(&owned.0).map_err(|_| "snapshot selection fixture cleanup".to_owned())?;
    owned.1 = true;
    Ok(())
}
#[cfg(not(unix))]
fn filesystem_controls(_: &Row, _: &[u8], _: &[u8]) -> Result<()> {
    Ok(())
}
/// Actual current positives for both rows run before any hostile input.
pub(super) fn controls(root: &Path) -> Result<()> {
    authenticated_module(root)?;
    let mut positives = Vec::new();
    for row in ROWS {
        let (raw, mode) = physical(root, row.path)?;
        let old = prior(root, row.path, row.prior_sha, &raw)?;
        need(
            old == historical(root, row)?,
            "snapshot selection positive prior",
        )?;
        positives.push((row, raw, mode, old));
    }
    for (row, raw, mode, old) in positives {
        let mut changed = raw.clone();
        changed[0] ^= 1;
        need(
            inverse(row, &changed, mode).is_err(),
            "snapshot selection tamper accepted",
        )?;
        need(
            prior(root, row.path, row.prior_sha, &changed).is_err(),
            "snapshot selection nonphysical operand accepted",
        )?;
        need(
            inverse(row, &old, mode).is_err(),
            "snapshot selection old current accepted",
        )?;
        need(
            inverse(row, &raw, 0o600).is_err(),
            "snapshot selection wrong mode accepted",
        )?;
        need(
            prior(root, row.path, "wrong", &raw).is_err(),
            "snapshot selection wrong caller accepted",
        )?;
        filesystem_controls(row, &raw, &old)?;
    }
    need(
        prior(root, "unlisted.rs", "wrong", b"").is_err(),
        "snapshot selection unlisted path accepted",
    )
}
