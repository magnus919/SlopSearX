//! xtask-only controls; never included by historical A13 fixture binaries.

use super::storage_document::{archive_path, is_path, verify, verify_historical, verify_successor};
const BINDINGS: &[(&str, &str)] = &[
    (
        "AGENTS.md",
        "d710c84ebad4582dc607cc3bdb806a5a5253259e4cabd5c515f51e55c3588af8",
    ),
    (
        "CONTRIBUTING.md",
        "2df6be141a3b06c662a4062af79eb074248ea70f297a7bf4c65432c10feba681",
    ),
];
fn archive(path: &str) -> &'static [u8] {
    match path {
        "AGENTS.md" => include_bytes!("AGENTS.before.md"),
        "CONTRIBUTING.md" => include_bytes!("CONTRIBUTING.before.md"),
        _ => panic!("test fixture path"),
    }
}
fn current(path: &str) -> &'static [u8] {
    match path {
        "AGENTS.md" => include_bytes!("../../../AGENTS.md"),
        "CONTRIBUTING.md" => include_bytes!("../../../CONTRIBUTING.md"),
        _ => panic!("test fixture path"),
    }
}
#[test]
fn exact_current_and_original_archives_are_accepted() {
    for (path, original) in BINDINGS {
        assert!(is_path(path));
        assert!(
            archive_path(path)
                .unwrap()
                .starts_with("docs/rust-rewrite/storage-document-amendment/")
        );
        verify(path, original, None, current(path), archive(path)).unwrap();
        verify(
            path,
            original,
            Some(archive(path).len()),
            current(path),
            archive(path),
        )
        .unwrap();
    }
    let baseline = include_bytes!("AGENTS.accepted-504.md.frozen");
    let capsule = include_bytes!("agents-successor-v2.json");
    let original = BINDINGS[0].1;
    let accepted_sha = "7d681485699a897464cf248d22fa83bd81e7b85d96a0163f6c9dabc019f988f6";
    verify_historical("AGENTS.md", original, None, baseline, archive("AGENTS.md")).unwrap();
    verify_successor(
        "AGENTS.md",
        accepted_sha,
        baseline,
        current("AGENTS.md"),
        capsule,
    )
    .unwrap();
    // Accepted504 is historical evidence; it cannot pass as live current.
    assert!(verify("AGENTS.md", original, None, baseline, archive("AGENTS.md")).is_err());
    assert!(
        verify_historical(
            "AGENTS.md",
            original,
            None,
            current("AGENTS.md"),
            archive("AGENTS.md")
        )
        .is_err()
    );
    let mut wrong_baseline = baseline.to_vec();
    wrong_baseline.push(0);
    assert!(
        verify_successor(
            "AGENTS.md",
            accepted_sha,
            &wrong_baseline,
            current("AGENTS.md"),
            capsule
        )
        .is_err()
    );
    assert!(
        verify_successor(
            "AGENTS.md",
            "wrong-caller",
            baseline,
            current("AGENTS.md"),
            capsule
        )
        .is_err()
    );
    assert!(
        verify_successor(
            "CONTRIBUTING.md",
            accepted_sha,
            baseline,
            current("AGENTS.md"),
            capsule
        )
        .is_err()
    );
    let mut wrong_current = current("AGENTS.md").to_vec();
    wrong_current.push(0);
    assert!(
        verify_successor("AGENTS.md", accepted_sha, baseline, &wrong_current, capsule).is_err()
    );
    let mut wrong_capsule = capsule.to_vec();
    wrong_capsule.push(0);
    assert!(
        verify_successor(
            "AGENTS.md",
            accepted_sha,
            baseline,
            current("AGENTS.md"),
            &wrong_capsule
        )
        .is_err()
    );
    println!(
        "STORAGE_AGENTS_SUCCESSOR_CONTROLS historical504_separate_actual_current_reconstructed_old_as_current_wrong_archive_caller_and_current_rejected=true"
    );
}
#[test]
fn wrong_original_hash_or_size_is_rejected() {
    for (path, original) in BINDINGS {
        assert!(verify(path, "wrong-original", None, current(path), archive(path)).is_err());
        assert!(
            verify(
                path,
                original,
                Some(archive(path).len() + 1),
                current(path),
                archive(path)
            )
            .is_err()
        );
    }
}
#[test]
fn tampered_current_bytes_are_rejected() {
    for (path, original) in BINDINGS {
        let mut changed = current(path).to_vec();
        changed.push(b' ');
        assert!(verify(path, original, None, &changed, archive(path)).is_err());
    }
}
#[test]
fn tampered_historical_archive_is_rejected() {
    for (path, original) in BINDINGS {
        let mut changed = archive(path).to_vec();
        changed.push(b' ');
        assert!(verify(path, original, None, current(path), &changed).is_err());
    }
}
#[test]
fn unknown_or_prefix_paths_are_rejected() {
    for path in [
        "README.md",
        "AGENTS.md/extra",
        "./AGENTS.md",
        "docs/CONTRIBUTING.md",
    ] {
        assert!(!is_path(path));
        assert!(archive_path(path).is_err());
        assert!(
            verify(
                path,
                BINDINGS[0].1,
                None,
                current("AGENTS.md"),
                archive("AGENTS.md")
            )
            .is_err()
        );
    }
}
#[test]
fn all_three_immutable_coverage_containers_retain_both_original_doc_bindings() {
    for raw in [
        include_str!("../a07/coverage.json"),
        include_str!("../a08/coverage.json"),
        include_str!("../a09/coverage.json"),
    ] {
        let coverage: serde_json::Value = serde_json::from_str(raw).unwrap();
        let baseline = coverage["baseline"].as_str().unwrap();
        assert_eq!(baseline.len(), 40);
        assert!(baseline.bytes().all(|byte| byte.is_ascii_hexdigit()));
        let mut seen = std::collections::BTreeSet::new();
        for row in coverage["source_inventory"].as_array().unwrap() {
            let path = row["path"].as_str().unwrap();
            if is_path(path) {
                assert!(seen.insert(path));
                verify(
                    path,
                    row["sha256"].as_str().unwrap(),
                    row["bytes"]
                        .as_u64()
                        .map(|bytes| usize::try_from(bytes).unwrap()),
                    current(path),
                    archive(path),
                )
                .unwrap();
            }
        }
        assert_eq!(
            seen,
            std::collections::BTreeSet::from(["AGENTS.md", "CONTRIBUTING.md"])
        );
    }
}
