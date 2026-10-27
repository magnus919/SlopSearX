//! Pure source-provenance acceptance through the original wrapped A13 verifier.
//! All thirteen frozen caller tuples and their negative controls run without
//! provider access, private Valkey, or rewritten historical files.
#[allow(dead_code)]
#[path = "../../../docs/rust-rewrite/a13/source-amendments/validator.rs"]
mod source_amendment;

#[test]
fn native_catalog_thirteen_original_callers_and_negative_controls() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let capsule: serde_json::Value = serde_json::from_slice(
        &std::fs::read(root.join("docs/rust-rewrite/native-engines/catalog-source-amendment.json"))
            .unwrap(),
    )
    .unwrap();
    let bindings = capsule["bindings"].as_array().unwrap();
    assert_eq!(bindings.len(), 13);
    // Exercise the exact public alias used by original A13 consumers, as well
    // as the structural and wrong-caller controls below.
    for binding in bindings {
        let source = &binding["source_row"];
        source_amendment::verify_current(
            &root,
            binding["container"]["path"].as_str().unwrap(),
            binding["container"]["sha256"].as_str().unwrap(),
            source["path"].as_str().unwrap(),
            source["sha256"].as_str().unwrap(),
            source["bytes"]
                .as_u64()
                .map(|bytes| usize::try_from(bytes).unwrap()),
        )
        .unwrap();
    }
    source_amendment::causal_successor::a06_source_successor::native_catalog_source_controls(&root)
        .unwrap();
}
