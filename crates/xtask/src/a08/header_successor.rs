//! Finite default-User-Agent successor; original A08/F08 evidence is immutable.
use super::{digest, require};
use crate::Result;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};
const CAPSULE: &str =
    "docs/rust-rewrite/a08/source-amendments/default-user-agent-successor-v1.json";
const CAPSULE_SHA: &str = "d89b13c6255157959eeaed232edd7fc38408cca1d9d2cbf959a9142f3483416c";
const EFFECTIVE: &str =
    "docs/rust-rewrite/a08/source-amendments/default-user-agent-effective-v1.json";
const A07: &str = "docs/rust-rewrite/a07/source-amendments/pure-a06-composition-v1.json";
const A07_SHA: &str = "58cfde8aa2635d2150792a386c2f0b4570ca544d869967c6c2f80bf9e1c3a0ec";
const UA: &str = concat!(
    "SlopSearX/",
    env!("CARGO_PKG_VERSION"),
    " (meta search engine; agent-native)"
);
const EDGAR: &str = "SlopSearX/0.1.0 (meta search engine; research)";
const ADDED_TEST: &str =
    "native_user_agent_socket_precedence_redirect_and_session_identity_isolation";
const SCOPE: &str = "Exact frozen A08 row identities/source values with the reviewed versioned native default User-Agent; explicit Edgar request overrides and every non-UA request/result property remain outside this delta.";
const DIFFERENCE: &str = "Reviewed native default identity replaces absent native User-Agent only; source HTTPX identity, request/session overrides, Edgar override, and all non-UA properties remain independently asserted. No provider compatibility or full transport-header parity claim.";
const FIXED: &[(&str, &str, u64)] = &[
    (
        "docs/rust-rewrite/a08/coverage.json",
        "4bd65113eded80d605b3fede32f9cb5e97d12b47c4b1756314ed28757f653856",
        47496,
    ),
    (
        "docs/rust-rewrite/a08/header-differences.json",
        "a1034125c9ff8047e85559537a29be85872d9c04a5ee057fa21739e1d71904f8",
        92194,
    ),
    (
        "docs/rust-rewrite/f08/coverage.json",
        "f37c387f06fd266870a679ba7fb1e92439c5c1ba8f4a0891a4f14871ba1f9634",
        2594,
    ),
    (
        "docs/rust-rewrite/a08/source.json",
        "a16bcf6bde89876645cb76f4a7a68c7d0cd96795ead4e77f971190184f6b8539",
        505466,
    ),
    (
        "docs/rust-rewrite/a08/regressions.json",
        "d4fc84fd63b856a3a7f8559a8a8a6565e171383a79294788f72c660aa7fe2d15",
        98752,
    ),
    (
        "docs/rust-rewrite/a08/differences.json",
        "be4b89f76f48126fc7316684ce45cd5b8ad232036840e37dd602aac527029c34",
        65486,
    ),
    (
        "Cargo.toml",
        "d7383efcb512764e820ac778a5fed9b990fbf734ce75d525e077e7e905587dd3",
        1984,
    ),
    (
        "crates/ssx-infra/Cargo.toml",
        "349c3dc8db94a995d7eef27acbe8b5eeccb9f6cff1ccdd3e8308f638d301f51e",
        1254,
    ),
    (
        "crates/ssx-infra/src/adapters/edgar.rs",
        "b5646b3433ff302293162fbf31f8f0ffa58570697d7f5f35e182df4efe597eb5",
        11845,
    ),
    (
        "docs/rust-rewrite/a07/source-amendments/pure-a06-composition-v1.json",
        "58cfde8aa2635d2150792a386c2f0b4570ca544d869967c6c2f80bf9e1c3a0ec",
        64752,
    ),
    (
        "crates/ssx-infra/tests/support/default_ca_environment.rs",
        "00aee0178f3f111449e5a6e99f495538a1eafa29ec95ced7d8343a3a8ee1108c",
        959,
    ),
];
const BEFORE: &[(&str, &str, u64)] = &[
    (
        "crates/ssx-infra/tests/a08_factories.rs",
        "54dd24f4d93bf509ba7a8d6ce83b811e572ed385cabea4bfcc6dd4860301160a",
        28737,
    ),
    (
        "crates/ssx-infra/tests/http.rs",
        "e0f9fc2d7700e9069e18ad80d74e0e349d9901528c76137e4c3fe02ed297144a",
        44907,
    ),
    (
        "crates/ssx-infra/tests/a08_regulatory.rs",
        "8c7e0b4ada32143817ff19b2c755415a0dedcff41d494b7892cfd7cd5130e260",
        12392,
    ),
    (
        "docs/rust-rewrite/a08/README.md",
        "73452b2a6e66aa62958d5b7baa48b18136bca9b81b6e0330e76d482724b1ece7",
        7231,
    ),
];
#[derive(Clone)]
struct Material {
    bytes: Vec<u8>,
    mode: u32,
}
fn permission_bits(raw: u32) -> u32 {
    raw & 0o7777
}
fn load(root: &Path, path: &str) -> Result<Material> {
    let old = match path {
        "crates/ssx-infra/tests/a08_factories.rs" => Some((
            "329ff395b82e91eed0a0ab384367bbb745e1011812584391ce03f44210c24a92",
            28889,
        )),
        "crates/ssx-infra/tests/support/default_ca_environment.rs" => Some((
            "00aee0178f3f111449e5a6e99f495538a1eafa29ec95ced7d8343a3a8ee1108c",
            959,
        )),
        _ => None,
    };
    if let Some((sha, bytes)) = old {
        return Ok(Material {
            bytes: crate::a13::a06_source_successor::default_ca_fixture_operand(
                root,
                path,
                sha,
                Some(bytes),
            )
            .ok_or("A08 default-CA fixture tuple not selected")??,
            mode: 0o644,
        });
    }

    let p = root.join(path);
    let m = fs::symlink_metadata(&p).map_err(|e| e.to_string())?;
    require(
        m.file_type().is_file(),
        "UA proof requires regular material",
    )?;
    #[cfg(unix)]
    let mode = {
        use std::os::unix::fs::PermissionsExt;
        permission_bits(m.permissions().mode())
    };
    #[cfg(not(unix))]
    let mode = 0o644;
    Ok(Material {
        bytes: fs::read(p).map_err(|e| e.to_string())?,
        mode,
    })
}
fn str_field<'a>(v: &'a Value, key: &str) -> Result<&'a str> {
    v[key]
        .as_str()
        .ok_or_else(|| format!("UA proof missing {key}"))
}
fn rows<'a>(v: &'a Value, key: &str) -> Result<&'a Vec<Value>> {
    v[key]
        .as_array()
        .ok_or_else(|| format!("UA proof missing {key}"))
}
fn parse(raw: &[u8]) -> Result<Value> {
    require(digest(raw) == CAPSULE_SHA, "UA successor capsule changed")?;
    serde_json::from_slice(raw).map_err(|e| e.to_string())
}
fn json_material(m: &Material) -> Result<Value> {
    serde_json::from_slice(&m.bytes).map_err(|e| e.to_string())
}
fn material(row: &Value, m: &Material) -> Result<()> {
    require(
        row["sha256"].as_str() == Some(digest(&m.bytes).as_str())
            && row["bytes"].as_u64() == Some(m.bytes.len() as u64)
            && row["mode"] == json!(m.mode)
            && m.mode == 0o644,
        "UA proof material identity differs",
    )
}
fn reconstruct(old: &[u8], edits: &[Value]) -> Result<Vec<u8>> {
    require(!edits.is_empty(), "UA empty source transition")?;
    let mut cursor = 0;
    let mut previous = None;
    let mut result = Vec::new();
    for e in edits {
        require(
            e.as_object().is_some_and(|o| o.len() == 3),
            "UA source edit fields",
        )?;
        let at = usize::try_from(e["at"].as_u64().ok_or("UA source edit offset")?)
            .map_err(|_| "UA source offset overflow")?;
        require(
            previous.is_none_or(|p| at > p) && at >= cursor && at <= old.len(),
            "UA source edit order",
        )?;
        let removed = str_field(e, "remove_utf8")?.as_bytes();
        let inserted = str_field(e, "insert_utf8")?.as_bytes();
        require(removed != inserted, "UA source no-op edit")?;
        let end = at
            .checked_add(removed.len())
            .ok_or("UA source edit overflow")?;
        require(old.get(at..end) == Some(removed), "UA source edit preimage")?;
        result.extend_from_slice(&old[cursor..at]);
        result.extend_from_slice(inserted);
        cursor = end;
        previous = Some(at);
    }
    result.extend_from_slice(&old[cursor..]);
    Ok(result)
}
fn transition(
    t: &Value,
    expected: (&str, &str, u64),
    archive_prefix: &str,
    loader: &impl Fn(&str) -> Result<Material>,
) -> Result<()> {
    let (path, sha, bytes) = expected;
    require(
        t["before"] == json!({"path":path,"sha256":sha,"bytes":bytes,"mode":420}),
        "UA source predecessor tuple",
    )?;
    require(
        t["archive"]
            == json!({"path":format!("{archive_prefix}/{path}.frozen"),"sha256":sha,"bytes":bytes,"mode":420}),
        "UA source archive tuple",
    )?;
    let old = loader(str_field(&t["archive"], "path")?)?;
    material(&t["archive"], &old)?;
    require(t["after"]["path"] == path, "UA source successor scope")?;
    let current = loader(path)?;
    material(&t["after"], &current)?;
    require(
        reconstruct(&old.bytes, rows(t, "edits")?)? == current.bytes,
        "UA source reconstruction differs",
    )
}
fn derive_headers(baseline: &Value, source: &Value, regression: &Value) -> Result<Value> {
    let mut cases = BTreeMap::new();
    let mut edgar_requests = 0;
    for c in rows(source, "records")?
        .iter()
        .chain(rows(regression, "records")?)
    {
        require(
            cases.insert(str_field(c, "id")?, c).is_none(),
            "UA duplicate primary case",
        )?;
        if c["engine"] == "edgar" {
            for r in rows(c, "requests")? {
                require(
                    r["headers"]["user-agent"] == EDGAR,
                    "UA Edgar override changed",
                )?;
                edgar_requests += 1;
            }
        }
    }
    require(
        cases.len() == 387 && edgar_requests == 128,
        "UA original case/Edgar scope differs",
    )?;
    let old = rows(baseline, "records")?;
    require(old.len() == 232, "UA original header row count differs")?;
    let mut seen = BTreeSet::new();
    let mut request_values = 0;
    let mut derived = baseline.clone();
    derived["schema_version"] = json!(2);
    derived["scope"] = json!(SCOPE);
    derived["predecessor"] = json!({"path":"docs/rust-rewrite/a08/header-differences.json","sha256":"a1034125c9ff8047e85559537a29be85872d9c04a5ee057fa21739e1d71904f8"});
    for (row, current) in old.iter().zip(
        derived["records"]
            .as_array_mut()
            .ok_or("UA current rows missing")?,
    ) {
        let id = str_field(row, "id")?;
        require(seen.insert(id), "UA duplicate header row")?;
        let case = cases.get(id).ok_or("UA header row has no source case")?;
        require(
            ["fred", "openfda", "oyez"].contains(&str_field(case, "engine")?)
                && row["header"] == "user-agent",
            "UA amendment header/engine scope differs",
        )?;
        let expected: Vec<Value> = rows(case, "requests")?
            .iter()
            .map(|r| r["headers"]["user-agent"].clone())
            .collect();
        let native = rows(row, "native_values")?;
        require(
            !expected.is_empty()
                && expected.iter().all(|v| v == "python-httpx/0.28.1")
                && row["source_values"] == json!(expected)
                && native.len() == expected.len()
                && native.iter().all(Value::is_null),
            "UA baseline request identity differs",
        )?;
        request_values += native.len();
        current["native_values"] = json!(vec![UA; native.len()]);
        current["difference"] = json!(DIFFERENCE);
    }
    require(
        request_values == 240,
        "UA request-value cardinality differs",
    )?;
    Ok(derived)
}
fn effective_f08(baseline: &Value) -> Result<Value> {
    let mut result = baseline.clone();
    let tests = rows(baseline, "tests")?;
    let names: BTreeSet<_> = tests.iter().filter_map(Value::as_str).collect();
    require(
        tests.len() == 24
            && names.len() == 24
            && !names.contains(ADDED_TEST)
            && rows(baseline, "unit_tests")?.len() == 4,
        "UA frozen F08 inventory differs",
    )?;
    result["tests"]
        .as_array_mut()
        .ok_or("UA effective F08 tests missing")?
        .push(json!(ADDED_TEST));
    Ok(result)
}
fn validate(doc: &Value, loader: &impl Fn(&str) -> Result<Material>) -> Result<Value> {
    require(
        doc["schema"] == "slopsearx.a08-f08.default-user-agent-successor.v1",
        "UA successor schema",
    )?;
    let fixed = rows(doc, "fixed_sources")?;
    require(fixed.len() == FIXED.len(), "UA fixed source scope")?;
    for (r, (path, sha, bytes)) in fixed.iter().zip(FIXED) {
        require(
            *r == json!({"path":path,"sha256":sha,"bytes":bytes,"mode":420}),
            "UA fixed source tuple",
        )?;
        material(r, &loader(path)?)?;
    }
    let transitions = rows(doc, "transitions")?;
    require(
        transitions.len() == BEFORE.len(),
        "UA source transition scope",
    )?;
    for (t, expected) in transitions.iter().zip(BEFORE) {
        transition(
            t,
            *expected,
            "docs/rust-rewrite/a08/source-amendments/default-user-agent-v1/d561",
            loader,
        )?;
    }
    let a07 = json_material(&loader(A07)?)?;
    require(
        a07["schema"] == "slopsearx.a07.pure-a06-composition.v1",
        "UA HTTP composition schema",
    )?;
    let t = &rows(&a07, "transitions")?[0];
    let reference = &doc["http_transition_reference"];
    require(
        reference
            == &json!({"capsule_path":A07,"capsule_sha256":A07_SHA,"transition_index":0,"before":t["before"],"archive":t["archive"],"after":t["after"]}),
        "UA HTTP composition reference differs",
    )?;
    transition(
        t,
        (
            "crates/ssx-infra/src/http.rs",
            "c5116d9a0081a0497a12643440053f001b6d68b48e9b4bf8c5feabf81eed5d10",
            54774,
        ),
        "docs/rust-rewrite/a07/source-amendments/pure-a06-v1/d561",
        loader,
    )?;
    require(
        doc["scope"]["header"] == "user-agent"
            && doc["scope"]["original_row_count"] == 232
            && doc["scope"]["request_values"] == 240
            && doc["scope"]["native_default"] == UA
            && doc["scope"]["edgar_override"] == EDGAR
            && doc["f08_added_test"] == ADDED_TEST,
        "UA intended scope differs",
    )?;
    let expected = derive_headers(
        &json_material(&loader(FIXED[1].0)?)?,
        &json_material(&loader(FIXED[3].0)?)?,
        &json_material(&loader(FIXED[4].0)?)?,
    )?;
    let current = &doc["effective_headers"];
    require(current["path"] == EFFECTIVE, "UA effective artifact scope")?;
    let actual = loader(EFFECTIVE)?;
    material(current, &actual)?;
    require(
        json_material(&actual)? == expected,
        "UA effective headers differ from exact derivation",
    )?;
    effective_f08(&json_material(&loader(FIXED[2].0)?)?)
}
// Called only for the exact frozen README artifact tuple after verify() has
// reconstructed this current document from its exact archived main preimage.
pub(super) fn verify_readme_pin(pin: &Value, current: &[u8]) -> Result<()> {
    require(
        *pin == json!({"path":"docs/rust-rewrite/a08/README.md","sha256":"73452b2a6e66aa62958d5b7baa48b18136bca9b81b6e0330e76d482724b1ece7"}),
        "UA README predecessor pin differs",
    )?;
    require(
        current.len() == 7591
            && digest(current)
                == "6d183ec7f2d5852d9b963dbed8eb242a45ac4d60954998c6d69a34ff2c0495fd",
        "UA README current bytes differ",
    )
}
fn must_reject(result: Result<Value>, why: &str) -> Result<()> {
    require(result.is_err(), why)
}
fn controls(doc: &Value, materials: &BTreeMap<String, Material>) -> Result<()> {
    let check = |d: &Value, m: &BTreeMap<String, Material>| {
        validate(d, &|p| {
            m.get(p)
                .cloned()
                .ok_or("UA control missing material".into())
        })
    };
    check(doc, materials)?;
    #[cfg(any(unix, test))]
    {
        require(
            permission_bits(0o100644) == 0o644 && permission_bits(0o104644) == 0o4644,
            "UA permission extraction changed",
        )?;
    }
    for path in materials.keys() {
        let mut missing = materials.clone();
        missing.remove(path);
        must_reject(check(doc, &missing), "UA missing material accepted")?;
        let mut changed = materials.clone();
        changed
            .get_mut(path)
            .ok_or("UA control material missing")?
            .bytes
            .push(b'!');
        must_reject(check(doc, &changed), "UA altered material accepted")?;
        for mode in [0o600, 0o4644] {
            let mut changed = materials.clone();
            changed
                .get_mut(path)
                .ok_or("UA mode material missing")?
                .mode = permission_bits(0o100000 | mode);
            must_reject(check(doc, &changed), "UA altered mode accepted")?;
        }
    }
    for index in 0..BEFORE.len() {
        let path = BEFORE[index].0;
        let archive = str_field(&doc["transitions"][index]["archive"], "path")?;
        let mut rollback = materials.clone();
        rollback.insert(path.into(), materials[archive].clone());
        must_reject(check(doc, &rollback), "UA source rollback accepted")?;
        for use_archive in [false, true] {
            let mut changed = materials.clone();
            if use_archive {
                changed.insert(path.into(), materials[archive].clone());
            } else {
                changed
                    .get_mut(path)
                    .ok_or("UA control source missing")?
                    .bytes
                    .push(b'!');
            }
            let mut rehashed = doc.clone();
            let b = &changed[path].bytes;
            rehashed["transitions"][index]["after"]["sha256"] = json!(digest(b));
            rehashed["transitions"][index]["after"]["bytes"] = json!(b.len());
            require(
                check(&rehashed, &changed).err().as_deref()
                    == Some("UA source reconstruction differs"),
                "UA rehashed source bypassed reconstruction",
            )?;
        }
    }
    for field in ["fixed_sources", "transitions"] {
        let mut wrong = doc.clone();
        wrong[field]
            .as_array_mut()
            .ok_or("UA control rows missing")?
            .pop();
        must_reject(check(&wrong, materials), "UA missing source tuple accepted")?;
        let mut extra = doc.clone();
        let first = extra[field][0].clone();
        extra[field]
            .as_array_mut()
            .ok_or("UA control rows missing")?
            .push(first);
        must_reject(check(&extra, materials), "UA extra source tuple accepted")?;
    }
    for kind in ["missing", "duplicate", "moved", "preimage", "insert"] {
        let mut wrong = doc.clone();
        let edits = wrong["transitions"][0]["edits"]
            .as_array_mut()
            .ok_or("UA control edits missing")?;
        match kind {
            "missing" => {
                edits.remove(0);
            }
            "duplicate" => {
                let first = edits[0].clone();
                edits.insert(1, first);
            }
            "moved" => {
                edits.swap(0, 1);
            }
            "preimage" => {
                edits[0]["remove_utf8"] = json!("wrong preimage");
            }
            _ => {
                edits[0]["insert_utf8"] = json!("unlisted change");
            }
        }
        let expected = match kind {
            "duplicate" | "moved" => "UA source edit order",
            "preimage" => "UA source edit preimage",
            _ => "UA source reconstruction differs",
        };
        require(
            check(&wrong, materials).err().as_deref() == Some(expected),
            "UA structural mutation bypassed reconstruction guard",
        )?;
    }
    for kind in [
        "baseline", "missing", "extra", "default", "source", "header", "id", "cookie",
    ] {
        let mut current = json_material(&materials[EFFECTIVE])?;
        match kind {
            "baseline" => {
                current = json_material(&materials[FIXED[1].0])?;
            }
            "missing" => {
                current["records"]
                    .as_array_mut()
                    .ok_or("UA mutation rows missing")?
                    .pop();
            }
            "extra" => {
                let row = current["records"][0].clone();
                current["records"]
                    .as_array_mut()
                    .ok_or("UA mutation rows missing")?
                    .push(row);
            }
            "default" => {
                current["records"][0]["native_values"][0] = Value::Null;
            }
            "source" => {
                current["records"][0]["source_values"][0] = json!("overridden-source-agent");
            }
            "header" => {
                current["records"][0]["header"] = json!("authorization");
            }
            "id" => {
                current["records"][0]["id"] = json!("edgar-success");
            }
            _ => {
                current["records"][0]["cookie"] = json!("unreviewed");
            }
        }
        let mut changed = materials.clone();
        let bytes = serde_json::to_vec(&current).map_err(|e| e.to_string())?;
        let mut rehashed = doc.clone();
        rehashed["effective_headers"]["sha256"] = json!(digest(&bytes));
        rehashed["effective_headers"]["bytes"] = json!(bytes.len());
        changed.insert(EFFECTIVE.into(), Material { bytes, mode: 0o644 });
        require(
            check(&rehashed, &changed).err().as_deref()
                == Some("UA effective headers differ from exact derivation"),
            "UA rehashed header-scope mutation accepted",
        )?;
    }
    let baseline_headers = json_material(&materials[FIXED[1].0])?;
    let original_source = json_material(&materials[FIXED[3].0])?;
    let original_regression = json_material(&materials[FIXED[4].0])?;
    let mut overridden = original_source.clone();
    let edgar = overridden["records"]
        .as_array_mut()
        .ok_or("UA original source rows missing")?
        .iter_mut()
        .find(|r| r["engine"] == "edgar" && r["requests"].as_array().is_some_and(|r| !r.is_empty()))
        .ok_or("UA Edgar mutation case missing")?;
    edgar["requests"][0]["headers"]["user-agent"] = json!(UA);
    require(
        derive_headers(&baseline_headers, &overridden, &original_regression)
            .err()
            .as_deref()
            == Some("UA Edgar override changed"),
        "UA default incorrectly replaces Edgar override",
    )?;
    for field in ["header", "source_values"] {
        let mut overridden = baseline_headers.clone();
        if field == "header" {
            overridden["records"][0][field] = json!("authorization");
        } else {
            overridden["records"][0][field][0] = json!("overridden-source-agent");
        }
        require(
            derive_headers(&overridden, &original_source, &original_regression).is_err(),
            "UA derivation accepts header/source scope changes",
        )?;
    }
    let original_f08 = json_material(&materials[FIXED[2].0])?;
    let effective = effective_f08(&original_f08)?;
    require(
        effective["unit_tests"] == original_f08["unit_tests"] && effective_f08(&effective).is_err(),
        "UA F08 baseline/current substitution accepted",
    )?;
    let readme_pin = json!({"path":"docs/rust-rewrite/a08/README.md","sha256":"73452b2a6e66aa62958d5b7baa48b18136bca9b81b6e0330e76d482724b1ece7"});
    let readme = &materials["docs/rust-rewrite/a08/README.md"].bytes;
    verify_readme_pin(&readme_pin, readme)?;
    for pin in [
        json!({"path":"docs/rust-rewrite/a08/README.md","sha256":"unreviewed"}),
        json!({"path":"another-document.md","sha256":"73452b2a6e66aa62958d5b7baa48b18136bca9b81b6e0330e76d482724b1ece7"}),
        json!({"path":"docs/rust-rewrite/a08/README.md","sha256":"73452b2a6e66aa62958d5b7baa48b18136bca9b81b6e0330e76d482724b1ece7","bytes":0}),
    ] {
        require(
            verify_readme_pin(&pin, readme).is_err(),
            "UA broadened README pin accepted",
        )?;
    }
    let archive = "docs/rust-rewrite/a08/source-amendments/default-user-agent-v1/d561/docs/rust-rewrite/a08/README.md.frozen";
    require(
        verify_readme_pin(&readme_pin, &materials[archive].bytes).is_err(),
        "UA README historical/current substitution accepted",
    )?;
    let mut wrong = doc.clone();
    wrong["scope"]["header"] = json!("all-headers");
    must_reject(
        check(&wrong, materials),
        "UA expanded header scope accepted",
    )?;
    let mut wrong = doc.clone();
    wrong["f08_added_test"] = json!("unrelated");
    must_reject(
        check(&wrong, materials),
        "UA unrelated F08 identity accepted",
    )?;
    Ok(())
}
pub(crate) fn verify(root: &Path, records: &mut Vec<Value>) -> Result<Value> {
    let capsule = load(root, CAPSULE)?;
    require(capsule.mode == 0o644, "UA capsule mode")?;
    let doc = parse(&capsule.bytes)?;
    let mut materials = BTreeMap::new();
    for row in rows(&doc, "fixed_sources")? {
        let path = str_field(row, "path")?;
        materials.insert(path.to_owned(), load(root, path)?);
    }
    for t in rows(&doc, "transitions")? {
        for key in ["archive", "after"] {
            let path = str_field(&t[key], "path")?;
            materials.insert(path.to_owned(), load(root, path)?);
        }
    }
    for row in [
        &doc["http_transition_reference"]["archive"],
        &doc["http_transition_reference"]["after"],
        &doc["effective_headers"],
    ] {
        let path = str_field(row, "path")?;
        let material = if path == "crates/ssx-infra/src/http.rs" {
            Material {
                bytes: crate::a13::a06_source_successor::real_clock_http_prior(
                    root,
                    crate::a13::a06_source_successor::RealClockHttpCaller::A08,
                )?,
                mode: 0o644,
            }
        } else {
            load(root, path)?
        };
        materials.insert(path.to_owned(), material);
    }
    let effective = validate(&doc, &|p| {
        materials
            .get(p)
            .cloned()
            .ok_or("UA material missing".into())
    })?;
    let mut altered = capsule.bytes.clone();
    altered.push(b'\n');
    require(parse(&altered).is_err(), "UA changed capsule accepted")?;
    controls(&doc, &materials)?;
    records.push(json!({"A08_F08_default_user_agent_successor":{"capsule":CAPSULE,"sha256":CAPSULE_SHA,"effective_headers":EFFECTIVE,"rows":232,"requests":240,"frozen_F08_tests":24,"effective_F08_tests":25,"worker_tests":4,"mandatory_rejection_controls":"passed","scope":"only reviewed default UA; exact source/Edgar override/TLS/proxy/auth/cookie/precedence controls retained; no provider parity claim"}}));
    Ok(effective)
}
