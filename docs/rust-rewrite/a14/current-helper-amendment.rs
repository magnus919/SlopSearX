//! A14's one reviewed helper amendment; shared A13 validation remains authoritative.
use crate::{Result, command};
use serde_json::{Value, json};
use std::{fs, path::Path};

pub const SOURCE_PATH: &str = "crates/ssx-infra/src/adapters/clinicaltrials.rs";
const AMENDMENT_PATH: &str = "docs/rust-rewrite/a14/current-helper-amendment-v1.json";
const AMENDMENT_SHA: &str = "75c1bb01e93e16c5e66139a6c0b95883acb9e9377fcf58e68813e71edf99db34";

#[derive(Clone, Copy)]
struct Payload<'a> {
    path: &'a str,
    accepted_blob: &'a str,
    archived_blob: &'a str,
    current_blob: &'a str,
    cases: &'a [u8],
    archive: &'a [u8],
    current: &'a [u8],
}

fn require(ok: bool, message: &str) -> Result<()> {
    if ok { Ok(()) } else { Err(message.into()) }
}

fn section<'a>(bytes: &'a [u8], start: &str, end: &str) -> Result<&'a [u8]> {
    let source = std::str::from_utf8(bytes).map_err(|e| e.to_string())?;
    require(
        source.matches(start).count() == 1,
        "A14 reference section start is missing or ambiguous",
    )?;
    let offset = source.find(start).ok_or("A14 reference section absent")?;
    let tail = &source[offset..];
    require(
        tail.matches(end).count() == 1,
        "A14 reference section end is missing or ambiguous",
    )?;
    let length = tail.find(end).ok_or("A14 reference section end absent")?;
    Ok(&bytes[offset..offset + length])
}

fn verify_sections(manifest: &Value, archive: &[u8], current: &[u8]) -> Result<()> {
    let sections = manifest["preserved_reference_sections"]
        .as_array()
        .ok_or("A14 reference sections absent")?;
    require(sections.len() == 2, "A14 reference section scope differs")?;
    for row in sections {
        let start = row["start"].as_str().ok_or("A14 section start absent")?;
        let end = row["end"].as_str().ok_or("A14 section end absent")?;
        let old = section(archive, start, end)?;
        let new = section(current, start, end)?;
        require(
            old == new
                && row["bytes"].as_u64() == Some(old.len() as u64)
                && row["sha256"] == super::digest(old),
            "A14 accepted timeout reference semantics changed",
        )?;
    }
    Ok(())
}

fn verify_payload(raw: &[u8], payload: Payload<'_>) -> Result<Value> {
    require(
        super::digest(raw) == AMENDMENT_SHA,
        "A14 reviewed current-helper amendment changed",
    )?;
    let manifest: Value = serde_json::from_slice(raw).map_err(|e| e.to_string())?;
    require(
        manifest["kind"] == "A14_ACCEPTED_HELPER_CURRENT_AMENDMENT_V1"
            && manifest["source_path"] == SOURCE_PATH
            && payload.path == SOURCE_PATH
            && manifest["approved_cases"]["path"] == "docs/rust-rewrite/a14/cases.json"
            && manifest["approved_cases"]["sha256"] == super::CASES_SHA
            && super::digest(payload.cases) == super::CASES_SHA,
        "A14 current-helper source/cases scope differs",
    )?;
    let cases: Value = serde_json::from_slice(payload.cases).map_err(|e| e.to_string())?;
    require(
        cases["accepted_helper_blobs"][SOURCE_PATH] == payload.accepted_blob
            && manifest["historical"]["git_blob"] == payload.accepted_blob
            && payload.archived_blob == payload.accepted_blob
            && manifest["current"]["git_blob"] == payload.current_blob,
        "A14 original archive or current helper blob differs",
    )?;
    require(
        manifest["historical"]["bytes"].as_u64() == Some(payload.archive.len() as u64)
            && manifest["historical"]["sha256"] == super::digest(payload.archive)
            && manifest["current"]["bytes"].as_u64() == Some(payload.current.len() as u64)
            && manifest["current"]["sha256"] == super::digest(payload.current),
        "A14 archived/current helper bytes differ",
    )?;
    verify_sections(&manifest, payload.archive, payload.current)?;
    Ok(manifest)
}

pub fn verify(
    root: &Path,
    path: &str,
    accepted_blob: &str,
    current_blob: &str,
    records: &mut Vec<Value>,
) -> Result<()> {
    let raw = fs::read(root.join(AMENDMENT_PATH)).map_err(|e| e.to_string())?;
    require(
        super::digest(&raw) == AMENDMENT_SHA,
        "A14 current-helper amendment pin differs",
    )?;
    let manifest: Value = serde_json::from_slice(&raw).map_err(|e| e.to_string())?;
    let archive_path = manifest["historical"]["archive_path"]
        .as_str()
        .ok_or("A14 historical helper archive absent")?;
    let archive = fs::read(root.join(archive_path)).map_err(|e| e.to_string())?;
    let current = fs::read(root.join(path)).map_err(|e| e.to_string())?;
    let cases =
        fs::read(root.join("docs/rust-rewrite/a14/cases.json")).map_err(|e| e.to_string())?;
    let archived_blob = command(root, "git", &["hash-object", "--", archive_path], records)?;
    let manifest = verify_payload(
        &raw,
        Payload {
            path,
            accepted_blob,
            archived_blob: archived_blob.trim(),
            current_blob,
            cases: &cases,
            archive: &archive,
            current: &current,
        },
    )?;
    let delegate = &manifest["strict_delegate"];
    let predecessor = fs::read(
        root.join(
            delegate["manifest_path"]
                .as_str()
                .ok_or("A14 A13 predecessor path absent")?,
        ),
    )
    .map_err(|e| e.to_string())?;
    require(
        delegate["manifest_sha256"] == super::digest(&predecessor),
        "A14 accepted A13 amendment predecessor changed",
    )?;
    crate::a13::check_source_pin(
        root,
        delegate["container_path"]
            .as_str()
            .ok_or("A14 delegate container absent")?,
        delegate["container_sha256"]
            .as_str()
            .ok_or("A14 delegate container SHA absent")?,
        path,
        delegate["historical_sha256"]
            .as_str()
            .ok_or("A14 delegate historical SHA absent")?,
        delegate["historical_bytes"].as_u64().map(|v| v as usize),
    )
    .map_err(|e| format!("A14 strict A13 current-helper delegation failed: {e}"))?;
    records.push(json!({
        "A14_current_helper_amendment": AMENDMENT_SHA,
        "original_accepted_helper_blob": accepted_blob,
        "verified_current_helper_blob": current_blob,
        "historical_archive": archive_path,
        "preserved_timeout_reference_sections": 2,
        "original_cases_and_captures": "unchanged",
    }));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>) {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap();
        (
            fs::read(root.join(AMENDMENT_PATH)).unwrap(),
            fs::read(root.join("docs/rust-rewrite/a14/cases.json")).unwrap(),
            fs::read(
                root.join("docs/rust-rewrite/a13/source-amendments/clinicaltrials.rs.9f1a.frozen"),
            )
            .unwrap(),
            // The original manifest's current side is accepted main, not the
            // new A06 candidate. Keep its positive and every mutant historical.
            fs::read(root.join(
                "docs/rust-rewrite/a06/source-amendments/main-d561/crates/ssx-infra/src/adapters/clinicaltrials.rs",
            ))
            .unwrap(),
        )
    }

    #[test]
    fn original_cases_archive_current_and_unrelated_helper_mutations_are_rejected() {
        let (raw, cases, archive, current) = fixture();
        let good = Payload {
            path: SOURCE_PATH,
            accepted_blob: "f4cc578e117b3f75b56cd6a7994c3ae38eb61dae",
            archived_blob: "f4cc578e117b3f75b56cd6a7994c3ae38eb61dae",
            current_blob: "632e3048064ec92c6290754966abe0130c0753fd",
            cases: &cases,
            archive: &archive,
            current: &current,
        };
        assert!(verify_payload(&raw, good).is_ok());
        let mut wrong = cases.clone();
        wrong.push(0);
        assert!(
            verify_payload(
                &raw,
                Payload {
                    cases: &wrong,
                    ..good
                }
            )
            .is_err()
        );
        let mut wrong = archive.clone();
        wrong.push(0);
        assert!(
            verify_payload(
                &raw,
                Payload {
                    archive: &wrong,
                    ..good
                }
            )
            .is_err()
        );
        let mut wrong = current.clone();
        wrong.push(0);
        assert!(
            verify_payload(
                &raw,
                Payload {
                    current: &wrong,
                    ..good
                }
            )
            .is_err()
        );
        assert!(
            verify_payload(
                &raw,
                Payload {
                    path: "crates/ssx-infra/src/adapters/source_json.rs",
                    ..good
                }
            )
            .is_err()
        );
        assert!(
            verify_payload(
                &raw,
                Payload {
                    accepted_blob: "changed",
                    ..good
                }
            )
            .is_err()
        );
        assert!(
            verify_payload(
                &raw,
                Payload {
                    archived_blob: "changed",
                    ..good
                }
            )
            .is_err()
        );
        assert!(
            verify_payload(
                &raw,
                Payload {
                    current_blob: "changed",
                    ..good
                }
            )
            .is_err()
        );
        let mut wrong = raw.clone();
        wrong.push(0);
        assert!(verify_payload(&wrong, good).is_err());

        // Prove the actual candidate through the same typed Git bridge A14
        // uses in production; an archived positive cannot stand in for it.
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap();
        let actual = fs::read(root.join(SOURCE_PATH)).unwrap();
        assert!(
            verify_payload(
                &raw,
                Payload {
                    current: &actual,
                    ..good
                }
            )
            .is_err()
        );
        let manifest: Value = serde_json::from_slice(&raw).unwrap();
        verify_sections(&manifest, &archive, &actual).unwrap();
        let mut records = Vec::new();
        let actual_blob = command(
            root,
            "git",
            &["hash-object", "--", SOURCE_PATH],
            &mut records,
        )
        .unwrap();
        assert_ne!(actual_blob.trim(), good.current_blob);
        let mut shared = |path: &str,
                          cases_sha: &str,
                          accepted: &str,
                          current: &str,
                          wrong_archive: bool| {
            crate::a13::a06_source_successor::try_git(
                root,
                "docs/rust-rewrite/a14/cases.json",
                cases_sha,
                path,
                accepted,
                current,
                |archive_path| {
                    assert_eq!(
                        archive_path,
                        "docs/rust-rewrite/a06/source-amendments/accepted-git/f4cc578e117b3f75b56cd6a7994c3ae38eb61dae.rs.frozen"
                    );
                    let value = command(
                        root,
                        "git",
                        &["hash-object", "--", archive_path],
                        &mut records,
                    )?;
                    Ok(if wrong_archive {
                        "changed".into()
                    } else {
                        value
                    })
                },
            )
        };
        assert!(
            shared(
                SOURCE_PATH,
                super::super::CASES_SHA,
                good.accepted_blob,
                actual_blob.trim(),
                false
            )
            .unwrap()
            .is_ok()
        );
        assert!(
            shared(
                SOURCE_PATH,
                super::super::CASES_SHA,
                good.accepted_blob,
                "changed",
                false
            )
            .unwrap()
            .is_err()
        );
        assert!(
            shared(
                SOURCE_PATH,
                super::super::CASES_SHA,
                "changed",
                actual_blob.trim(),
                false
            )
            .unwrap()
            .is_err()
        );
        assert!(
            shared(
                SOURCE_PATH,
                "changed",
                good.accepted_blob,
                actual_blob.trim(),
                false
            )
            .unwrap()
            .is_err()
        );
        assert!(
            shared(
                SOURCE_PATH,
                super::super::CASES_SHA,
                good.accepted_blob,
                actual_blob.trim(),
                true
            )
            .unwrap()
            .is_err()
        );
        assert!(
            shared(
                SOURCE_PATH,
                super::super::CASES_SHA,
                good.accepted_blob,
                good.current_blob,
                false
            )
            .unwrap()
            .is_err()
        );
        assert!(
            shared(
                "crates/ssx-infra/src/adapters/edgar.rs",
                super::super::CASES_SHA,
                good.accepted_blob,
                actual_blob.trim(),
                false
            )
            .is_none()
        );
        assert!(!records.is_empty());
        eprintln!(
            "A14_SHARED_CURRENT_HELPER_CONTROLS original_main_positive_and_mutants=true actual_current=true wrong_blob_caller_container_archive_and_old_as_current_rejected=true"
        );
    }

    #[test]
    fn equal_but_changed_timeout_reference_sections_are_rejected() {
        let (raw, _, archive, current) = fixture();
        let manifest: Value = serde_json::from_slice(&raw).unwrap();
        verify_sections(&manifest, &archive, &current).unwrap();
        for (before, after) in [
            ("Ok(limit.min(configured))", "Ok(limit.max(configured))"),
            (
                "Ok(if *v { 1. } else { 0. })",
                "Ok(if *v { 2. } else { 0. })",
            ),
        ] {
            let old = std::str::from_utf8(&archive).unwrap();
            let new = std::str::from_utf8(&current).unwrap();
            assert_eq!(old.matches(before).count(), 1);
            assert_eq!(new.matches(before).count(), 1);
            // Mutate both sides equally: equality alone must not grant approval.
            let old = old.replacen(before, after, 1);
            let new = new.replacen(before, after, 1);
            assert!(verify_sections(&manifest, old.as_bytes(), new.as_bytes()).is_err());
        }
    }
}
