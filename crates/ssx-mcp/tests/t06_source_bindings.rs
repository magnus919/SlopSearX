//! Native source-drift gate: reads frozen bytes, never executes Python.
#[allow(dead_code)]
#[path = "../../../docs/rust-rewrite/a06/source-amendments/w07-evidence-successor.rs"]
mod w07_evidence_successor;
#[path = "../../../docs/rust-rewrite/a06/source-amendments/w07-t02-joint.rs"]
mod w07_t02_joint;
use sha2::{Digest, Sha256};
use std::path::Path;
#[path = "../../../docs/rust-rewrite/t06/source-amendments/lock_amendment.rs"]
mod lock_amendment;
#[path = "../../../docs/rust-rewrite/a06/source-amendments/native-search-reader.rs"]
mod native_search;
#[path = "../../../docs/rust-rewrite/t06/source-amendments/receipt-control-successor.rs"]
mod receipt_control;
#[test]
fn original_profiles_strategies_tool_surface_and_observations_match_reviewed_source_bindings() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    native_search::controls(&root).unwrap();
    let raw = include_bytes!("../../../docs/rust-rewrite/t06/source_bindings_v2.json");
    assert_eq!(
        Sha256::digest(raw)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>(),
        "11660374ae3fd544590cb014b532eda4d6f3a95ef8f0bc062b308bd7911fc771",
        "reviewed source binding container drift"
    );
    let bindings: serde_json::Value = serde_json::from_slice(raw).unwrap();
    for key in ["sources", "artifacts"] {
        for row in bindings[key].as_array().unwrap() {
            let name = row["file"].as_str().unwrap();
            if key == "sources" && name == lock_amendment::SOURCE_PATH {
                lock_amendment::verify(&root, name, row["sha256"].as_str().unwrap()).unwrap();
                continue;
            }
            let bytes = std::fs::read(root.join(name)).unwrap();
            assert_eq!(
                Sha256::digest(bytes)
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect::<String>(),
                row["sha256"].as_str().unwrap(),
                "reviewed source/capture drift: {name}"
            );
        }
    }
    for required in [
        "slopsearx/capabilities.py",
        "slopsearx/research.py",
        "slopsearx/jev.py",
        "slopsearx/mcp/security.py",
    ] {
        assert!(
            bindings["sources"]
                .as_array()
                .unwrap()
                .iter()
                .any(|row| row["file"] == required),
            "missing static profiles/strategies/topology binding"
        );
    }
    let native: serde_json::Value = serde_json::from_str(include_str!(
        "../../../docs/rust-rewrite/t06/native_fixture_bindings.json"
    ))
    .unwrap();
    let paths: std::collections::BTreeSet<_> = native["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["file"].as_str().unwrap())
        .collect();
    assert_eq!(
        paths,
        std::collections::BTreeSet::from([
            "crates/ssx-mcp/src/bin/search-qualification.rs",
            "crates/ssx-mcp/tests/t06_support/fixture.rs",
            "crates/ssx-mcp/tests/t06_support/observer.rs",
            "crates/ssx-mcp/tests/t06_support/server.rs",
            "crates/ssx-infra/src/search.rs",
            "crates/ssx-infra/src/search_deadline_drain_tests.rs",
            "crates/ssx-mcp/src/opaque.rs",
            "crates/ssx-mcp/tests/t06_support/deadline_clock.rs",
            "crates/ssx-mcp/tests/t06_support/wall_guard.rs",
            "crates/ssx-mcp/tests/t06_transport.rs",
        ])
    );
    let storage_raw = include_bytes!(
        "../../../docs/rust-rewrite/t06/storage-amendment/current-storage-amendment-v1.json"
    );
    let digest = |bytes: &[u8]| {
        Sha256::digest(bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    };
    assert_eq!(
        digest(storage_raw),
        "cc5ec0678ef0a54bc21068ad1ea2a230e0283356f3138cc4424b624b0acc6a23",
        "reviewed storage amendment drift"
    );
    let storage: serde_json::Value = serde_json::from_slice(storage_raw).unwrap();
    assert_eq!(storage["schema_version"], 1);
    assert_eq!(
        storage["file"],
        "crates/ssx-mcp/tests/t06_support/server.rs"
    );
    let archive =
        include_bytes!("../../../docs/rust-rewrite/t06/storage-amendment/server.rs.7f8d.frozen");
    assert_eq!(digest(archive), storage["before_sha256"].as_str().unwrap());
    w07_evidence_successor::controls(&root).unwrap();
    receipt_control::controls(&root).unwrap();
    for row in native["files"].as_array().unwrap() {
        let name = row["file"].as_str().unwrap();
        let expected = if row["file"] == storage["file"] {
            assert_eq!(row["sha256"], storage["before_sha256"]);
            storage["after_sha256"].as_str().unwrap()
        } else {
            row["sha256"].as_str().unwrap()
        };
        let actual = std::fs::read(root.join(name)).unwrap();
        let proven = if name == native_search::SOURCE {
            native_search::verify(&root, name, row["sha256"].as_str().unwrap(), &actual).unwrap();
            w07_evidence_successor::t06_native_search(&root, row, &actual).unwrap()
        } else if let Some(prior) = receipt_control::parent(&root, name, expected, &actual).unwrap()
        {
            prior
        } else {
            actual
        };
        assert_eq!(
            digest(&proven),
            expected,
            "reviewed native fixture drift: {name}"
        );
    }
    assert_eq!(unicode_normalization::UNICODE_VERSION, (16, 0, 0));
}
