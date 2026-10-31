//! Finite six-edge CookieJar/proxy child; preserves the exact historical DNS lock hop.
use super::super::{digest, edits, material, need, permissions, read};
use serde_json::{Value, json};
use std::path::Path;
const CAPSULE: &str =
    "docs/rust-rewrite/t07/source-amendments/gateway-cookie-proxy-lock-successor-v1.json";
const SHA: &str = "5f263afdf0766e00208850fe94d999654ce02a7288d76c3d2da822fe1aeb1f80";
const BYTES: u64 = 1544;
fn document(raw: &[u8]) -> Result<Value, String> {
    need(digest(raw) == SHA, "T07 cookie/proxy capsule digest")?;
    let doc: Value = serde_json::from_slice(raw).map_err(|e| e.to_string())?;
    need(
        doc["kind"] == "T07_GATEWAY_COOKIE_PROXY_LOCK_SUCCESSOR_V1"
            && doc["source_parent"] == "c6147891422f56e02ca36195ca866776d3a50f3e"
            && doc["paths"] == json!(["Cargo.lock"])
            && doc["prior"]["path"] == "Cargo.lock"
            && doc["current"]["path"] == "Cargo.lock"
            && doc["archive"]["path"]
                == "docs/rust-rewrite/t07/source-amendments/gateway-cookie-proxy-predecessor-v1/Cargo.lock"
            && doc["archive"]["source_path"] == "Cargo.lock",
        "T07 cookie/proxy closed lock scope",
    )?;
    Ok(doc)
}
fn load(root: &Path) -> Result<Value, String> {
    document(&read(
        root,
        &json!({"path":CAPSULE,"sha256":SHA,"bytes":BYTES,"mode":"0644"}),
    )?)
}
fn validate(
    doc: &Value,
    expected: &Value,
    before: &[u8],
    actual: &[u8],
    mode: &str,
) -> Result<(), String> {
    need(
        doc["prior"] == *expected,
        "T07 cookie/proxy exact predecessor caller",
    )?;
    material(&doc["prior"], before, "0644")?;
    material(&doc["current"], actual, mode)?;
    need(
        edits(before, &doc["edits"])? == actual,
        "T07 cookie/proxy ordered lock reconstruction",
    )
}
pub(super) fn archived_prior(root: &Path, expected: &Value) -> Result<Vec<u8>, String> {
    let doc = load(root)?;
    need(
        doc["prior"] == *expected,
        "T07 cookie/proxy exact predecessor caller",
    )?;
    read(root, &doc["archive"])
}
// Exact accepted T07 lock frame between the current isolation parent and
// this immutable cookie/proxy capsule. No other lock caller is admitted.
const CURRENT_PARENT_INVERSE: &str = r###"[{"at":293,"remove_utf8":"\n[[package]]\nname = \"ahash\"\nversion = \"0.8.12\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\nchecksum = \"5a15f179cd60c4584b8a8c596927aadc462e27f2ca70c04e0071964a73ba7a75\"\ndependencies = [\n \"cfg-if\",\n \"getrandom 0.3.3\",\n \"once_cell\",\n \"version_check\",\n \"zerocopy\",\n]\n","insert_utf8":""},{"at":7364,"remove_utf8":"\n[[package]]\nname = \"bitvec\"\nversion = \"1.0.1\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\nchecksum = \"1bc2832c24239b0141d5674bb9174f9d68a8b5b3f2753311927c172ca46f7e9c\"\ndependencies = [\n \"funty\",\n \"radium\",\n \"tap\",\n \"wyz\",\n]\n","insert_utf8":""},{"at":22178,"remove_utf8":"name = \"funty\"\nversion = \"2.0.0\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\nchecksum = \"e6d5a32815ae3f33302d95fdcb2ce17862f8c65363dcfd29360480ba1001fc9c\"\n\n[[package]]\n","insert_utf8":""},{"at":25335,"remove_utf8":" \"wasi 0.11.1+wasi-snapshot-preview1\",\n","insert_utf8":" \"wasi\",\n"},{"at":25391,"remove_utf8":"]\n\n[[package]]\nname = \"getrandom\"\nversion = \"0.3.3\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\nchecksum = \"26145e563e54f2cadc477553f1ec5ee650b00862f0a58bcd12cbdc5f0ea2d2f4\"\ndependencies = [\n \"cfg-if\",\n \"libc\",\n \"r-efi 5.2.0\",\n \"wasi 0.14.2+wasi-0.2.4\",\n","insert_utf8":""},{"at":25909,"remove_utf8":" \"r-efi 6.0.0\",\n","insert_utf8":" \"r-efi\",\n"},{"at":33570,"remove_utf8":"name = \"jiter\"\nversion = \"0.14.0\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\nchecksum = \"b6f3b5d3f84b36f4ad09fd1da896d23d9852a1aa86556578dd0289f43dce311d\"\ndependencies = [\n \"ahash\",\n \"bitvec\",\n \"lexical-parse-float\",\n \"num-bigint 0.4.8\",\n \"num-traits\",\n \"pyo3\",\n \"smallvec\",\n]\n\n[[package]]\n","insert_utf8":""},{"at":35653,"remove_utf8":"\n[[package]]\nname = \"lexical-parse-float\"\nversion = \"1.0.5\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\nchecksum = \"de6f9cb01fb0b08060209a057c048fcbab8717b4c1ecd2eac66ebfe39a65b0f2\"\ndependencies = [\n \"lexical-parse-integer\",\n \"lexical-util\",\n \"static_assertions\",\n]\n\n[[package]]\nname = \"lexical-parse-integer\"\nversion = \"1.0.5\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\nchecksum = \"72207aae22fc0a121ba7b6d479e42cbfea549af1479c3f3a4f12c70dd66df12e\"\ndependencies = [\n \"lexical-util\",\n \"static_assertions\",\n]\n\n[[package]]\nname = \"lexical-util\"\nversion = \"1.0.6\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\nchecksum = \"5a82e24bf537fd24c177ffbbdc6ebcc8d54732c35b50a3f28cc3f4e4c949a0b3\"\ndependencies = [\n \"static_assertions\",\n]\n","insert_utf8":""},{"at":39481,"remove_utf8":" \"wasi 0.11.1+wasi-snapshot-preview1\",\n","insert_utf8":" \"wasi\",\n"},{"at":46123,"remove_utf8":"name = \"portable-atomic\"\nversion = \"1.15.0\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\nchecksum = \"05c8b63e8d9609db387f0324918f81d68fe27748f084ef092fb35954d0539a85\"\n\n[[package]]\n","insert_utf8":""},{"at":48104,"remove_utf8":"]\n\n[[package]]\nname = \"pyo3\"\nversion = \"0.28.2\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\nchecksum = \"cf85e27e86080aafd5a22eae58a162e133a589551542b3e5cee4beb27e54f8e1\"\ndependencies = [\n \"libc\",\n \"num-bigint 0.4.8\",\n \"num-traits\",\n \"once_cell\",\n \"portable-atomic\",\n \"pyo3-build-config\",\n \"pyo3-ffi\",\n \"pyo3-macros\",\n]\n\n[[package]]\nname = \"pyo3-build-config\"\nversion = \"0.28.2\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\nchecksum = \"8bf94ee265674bf76c09fa430b0e99c26e319c945d96ca0d5a8215f31bf81cf7\"\ndependencies = [\n \"target-lexicon\",\n]\n\n[[package]]\nname = \"pyo3-ffi\"\nversion = \"0.28.2\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\nchecksum = \"491aa5fc66d8059dd44a75f4580a2962c1862a1c2945359db36f6c2818b748dc\"\ndependencies = [\n \"libc\",\n \"pyo3-build-config\",\n]\n\n[[package]]\nname = \"pyo3-macros\"\nversion = \"0.28.2\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\nchecksum = \"f5d671734e9d7a43449f8480f8b38115df67bef8d21f76837fa75ee7aaa5e52e\"\ndependencies = [\n \"proc-macro2\",\n \"pyo3-macros-backend\",\n \"quote\",\n \"syn 2.0.119\",\n]\n\n[[package]]\nname = \"pyo3-macros-backend\"\nversion = \"0.28.2\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\nchecksum = \"22faaa1ce6c430a1f71658760497291065e6450d7b5dc2bcf254d49f66ee700a\"\ndependencies = [\n \"heck\",\n \"proc-macro2\",\n \"pyo3-build-config\",\n \"quote\",\n \"syn 2.0.119\",\n","insert_utf8":""},{"at":51086,"remove_utf8":"version = \"5.2.0\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\nchecksum = \"74765f6d916ee2faa39bc8e68e4f3ed8949b48cccdac59983d287a7cb71ce9c5\"\n\n[[package]]\nname = \"r-efi\"\n","insert_utf8":""},{"at":51436,"remove_utf8":"\n[[package]]\nname = \"radium\"\nversion = \"0.7.0\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\nchecksum = \"dc33ff2d4973d518d823d61aa239014831e521c75da58e3df4840d3f47749d09\"\n","insert_utf8":""},{"at":55706,"remove_utf8":" \"ssx-source-codecs\",\n","insert_utf8":""},{"at":56402,"remove_utf8":" \"jiter\",\n","insert_utf8":""},{"at":69013,"remove_utf8":" \"serde-saphyr\",\n","insert_utf8":""},{"at":69138,"remove_utf8":" \"ssx-registry\",\n","insert_utf8":""},{"at":69999,"remove_utf8":" \"jiter\",\n","insert_utf8":""},{"at":70222,"remove_utf8":" \"ssx-native-ip-text\",\n","insert_utf8":""},{"at":70262,"remove_utf8":" \"ssx-windows-owner\",\n","insert_utf8":""},{"at":70405,"remove_utf8":"]\n\n[[package]]\nname = \"ssx-native-ip-text\"\nversion = \"0.1.0\"\ndependencies = [\n \"libc\",\n","insert_utf8":""},{"at":71055,"remove_utf8":" \"idna\",\n","insert_utf8":""},{"at":71190,"remove_utf8":"]\n\n[[package]]\nname = \"ssx-windows-owner\"\nversion = \"0.1.0\"\ndependencies = [\n \"sha2 0.11.1\",\n \"tokio\",\n \"windows-sys 0.61.2\",\n","insert_utf8":""},{"at":71772,"remove_utf8":"\n[[package]]\nname = \"static_assertions\"\nversion = \"1.1.0\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\nchecksum = \"a2eb9349b6444b326872e140eb1cf5e7c522154d69e7a0ffb0fb81c06b37543f\"\n","insert_utf8":""},{"at":74728,"remove_utf8":"\n[[package]]\nname = \"tap\"\nversion = \"1.0.1\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\nchecksum = \"55937e1799185b12863d447f42597ed69d9928686b8d88a1df17376a097d8369\"\n\n[[package]]\nname = \"target-lexicon\"\nversion = \"0.13.3\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\nchecksum = \"df7f62577c25e07834649fc3b39fafdc597c0a3527dc1c60129201ccfcbaa50c\"\n","insert_utf8":""},{"at":84462,"remove_utf8":"name = \"wasi\"\nversion = \"0.14.2+wasi-0.2.4\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\nchecksum = \"9683f9a5a998d873c0d21fcbe3c083009670149a8fab228644b8bd36b2c48cb3\"\ndependencies = [\n \"wit-bindgen-rt\",\n]\n\n[[package]]\n","insert_utf8":""},{"at":93206,"remove_utf8":"name = \"wit-bindgen-rt\"\nversion = \"0.39.0\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\nchecksum = \"6f42320e61fe2cfd34354ecb597f86f413484a798ba44a8ca1165c58d42da6c1\"\ndependencies = [\n \"bitflags\",\n]\n\n[[package]]\n","insert_utf8":""},{"at":93617,"remove_utf8":"\n[[package]]\nname = \"wyz\"\nversion = \"0.5.1\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\nchecksum = \"05f360fc0b24296329c78fda852a1e9ae82de9cf7b27dae4b7f62f118f77b9ed\"\ndependencies = [\n \"tap\",\n]\n","insert_utf8":""}]"###;
fn current_parent_inverse() -> Result<Value, String> {
    serde_json::from_str(CURRENT_PARENT_INVERSE).map_err(|e| e.to_string())
}
fn capsule_frame(
    expected: &Value,
    parent: &[u8],
    mode: &str,
    inverse: &Value,
) -> Result<Vec<u8>, String> {
    need(
        *expected
            == json!({"path":"Cargo.lock","bytes":91779,"sha256":"6d24703b44100d1b634ef9b3c32dbc516ef2cd07e9a9381a3ef7c474ddd3a61b","git_blob":"31531dde19d9c14cb08697bd76cb227a349be6e5","mode":"0644"})
            && mode == "0644"
            && parent.len() == 97529
            && digest(parent) == "ac68fe297f68ba653cf96b664f0738e26437c5db8519b02fb038a139f6776c2c",
        "T07 cookie/proxy exact parent/caller frame differs",
    )?;
    need(
        digest(&serde_json::to_vec(inverse).map_err(|e| e.to_string())?)
            == "ed883fe4b4fc4d77834f3b3c31fb1e3bfc37344226241ce6f3bc4c697001ce39",
        "T07 cookie/proxy closed 28-span inverse differs",
    )?;
    let prior = edits(parent, inverse)?;
    material(expected, &prior, "0644")?;
    Ok(prior)
}
pub(super) fn parent_frame(root: &Path, parent: &[u8]) -> Result<Vec<u8>, String> {
    let doc = load(root)?;
    let prior = capsule_frame(
        &doc["current"],
        parent,
        &permissions(&root.join("Cargo.lock"))?,
        &current_parent_inverse()?,
    )?;
    let git = std::process::Command::new("git")
        .args([
            "cat-file",
            "blob",
            "31531dde19d9c14cb08697bd76cb227a349be6e5",
        ])
        .current_dir(root)
        .output()
        .map_err(|e| e.to_string())?;
    need(
        git.status.success() && git.stdout == prior,
        "T07 cookie/proxy capsule Git blob differs",
    )?;
    Ok(prior)
}
pub(super) fn actual_bytes(
    root: &Path,
    expected: &Value,
    before: &[u8],
) -> Result<Vec<u8>, String> {
    let doc = load(root)?;
    let actual = super::super::main_isolation_current::parent(
        root,
        "Cargo.lock",
        None,
        super::super::main_isolation_current::Parent::T07,
    )?;
    let actual = parent_frame(root, &actual)?;
    validate(
        &doc,
        expected,
        before,
        &actual,
        &permissions(&root.join("Cargo.lock"))?,
    )?;
    Ok(actual)
}
#[cfg(test)]
pub(super) fn controls(root: &Path) -> Result<(), String> {
    let doc = load(root)?;
    let expected = &doc["prior"];
    let before = read(root, &doc["archive"])?;
    let actual = actual_bytes(root, expected, &before)?;
    let parent = super::super::main_isolation_current::parent(
        root,
        "Cargo.lock",
        None,
        super::super::main_isolation_current::Parent::T07,
    )?;
    let inverse = current_parent_inverse()?;
    need(
        capsule_frame(&doc["current"], &parent, "0644", &inverse)? == actual,
        "T07 cookie/proxy current-frame positive differs",
    )?;
    for key in ["path", "sha256", "bytes", "git_blob", "mode"] {
        let mut wrong = doc["current"].clone();
        wrong[key] = json!("unlisted");
        need(
            capsule_frame(&wrong, &parent, "0644", &inverse).is_err(),
            "T07 cookie/proxy parent caller mutation accepted",
        )?;
    }
    need(
        capsule_frame(&doc["current"], &parent, "0600", &inverse).is_err(),
        "T07 cookie/proxy parent mode accepted",
    )?;
    let mut bad_parent = parent.clone();
    bad_parent.push(0);
    need(
        capsule_frame(&doc["current"], &bad_parent, "0644", &inverse).is_err(),
        "T07 cookie/proxy altered parent accepted",
    )?;
    for key in ["at", "remove_utf8", "insert_utf8"] {
        let mut bad = inverse.clone();
        bad[0][key] = if key == "at" {
            json!(1)
        } else {
            json!("unlisted")
        };
        need(
            capsule_frame(&doc["current"], &parent, "0644", &bad).is_err(),
            "T07 cookie/proxy inverse span mutation accepted",
        )?;
    }
    let mut missing = inverse.clone();
    missing.as_array_mut().unwrap().pop();
    let mut duplicate = inverse.clone();
    duplicate.as_array_mut().unwrap().push(inverse[0].clone());
    let mut extra = inverse.clone();
    extra
        .as_array_mut()
        .unwrap()
        .push(json!({"at":97529,"remove_utf8":"","insert_utf8":"unlisted"}));
    for bad in [missing, duplicate, extra] {
        need(
            capsule_frame(&doc["current"], &parent, "0644", &bad).is_err(),
            "T07 cookie/proxy incomplete/extra inverse accepted",
        )?;
    }

    let mut wrong = expected.clone();
    wrong["path"] = json!("Cargo.unlisted");
    need(
        validate(&doc, &wrong, &before, &actual, "0644").is_err(),
        "T07 cookie/proxy wrong caller accepted",
    )?;
    let mut wrong_row = expected.clone();
    wrong_row["sha256"] = json!("changed");
    need(
        validate(&doc, &wrong_row, &before, &actual, "0644").is_err(),
        "T07 cookie/proxy wrong predecessor row accepted",
    )?;
    let mut bad = before.clone();
    bad.push(0);
    need(
        validate(&doc, expected, &bad, &actual, "0644").is_err(),
        "T07 cookie/proxy altered predecessor accepted",
    )?;
    let mut bad = actual.clone();
    bad.push(0);
    need(
        validate(&doc, expected, &before, &bad, "0644").is_err(),
        "T07 cookie/proxy unrelated lock delta accepted",
    )?;
    need(
        validate(&doc, expected, &before, &actual, "0755").is_err(),
        "T07 cookie/proxy changed mode accepted",
    )?;
    let mut changed = doc.clone();
    changed["current"]["sha256"] = json!(digest(&bad));
    changed["current"]["bytes"] = json!(bad.len());
    need(
        document(&serde_json::to_vec(&changed).map_err(|e| e.to_string())?).is_err(),
        "T07 cookie/proxy rehashed capsule accepted",
    )?;
    let mut missing = doc.clone();
    missing["edits"]
        .as_array_mut()
        .ok_or("T07 cookie/proxy edits")?
        .pop();
    need(
        validate(&missing, expected, &before, &actual, "0644").is_err(),
        "T07 cookie/proxy missing edge accepted",
    )?;
    let mut duplicate = doc.clone();
    let first = duplicate["edits"][0].clone();
    duplicate["edits"]
        .as_array_mut()
        .ok_or("T07 cookie/proxy edits")?
        .insert(0, first);
    need(
        validate(&duplicate, expected, &before, &actual, "0644").is_err(),
        "T07 cookie/proxy duplicate edge accepted",
    )?;
    let mut span = doc.clone();
    span["edits"][0]["at"] = json!(
        span["edits"][0]["at"]
            .as_u64()
            .ok_or("T07 cookie/proxy span")?
            + 1
    );
    need(
        validate(&span, expected, &before, &actual, "0644").is_err(),
        "T07 cookie/proxy tampered span accepted",
    )?;
    for (field, value) in [
        ("source_path", "Cargo.unlisted"),
        ("path", "unlisted/Cargo.lock"),
    ] {
        let mut source = doc.clone();
        source["archive"][field] = json!(value);
        need(
            document(&serde_json::to_vec(&source).map_err(|e| e.to_string())?).is_err(),
            "T07 cookie/proxy changed archive source accepted",
        )?;
    }
    for (from, to) in [("0.103.16", "0.103.15"), ("1.0.10", "1.0.9")] {
        let rollback = std::str::from_utf8(&actual)
            .map_err(|e| e.to_string())?
            .replacen(from, to, 1);
        need(
            validate(&doc, expected, &before, rollback.as_bytes(), "0644").is_err(),
            "T07 cookie/proxy TLS rollback accepted",
        )?;
    }
    Ok(())
}
