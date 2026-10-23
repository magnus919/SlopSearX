//! Exact09ee current material remains historical; this finite child is proved separately.
use super::{digest, edits, material, need, permissions, read, text};
use serde_json::{Value, json};
use std::path::Path;
const CAPSULE: &str = "docs/rust-rewrite/a06/source-amendments/real-clock-successor-v1.json";
const CAPSULE_SHA: &str = "303593cc4ffbe23c3adc4d071d466c02c872a3c3a9db624e6b012cbd59b55bba";
const CAPSULE_BYTES: usize = 63884;
const HTTP: &str = "crates/ssx-infra/src/http.rs";
const SUPPORT: &str = "crates/ssx-infra/tests/support/http_expired_startup.rs";
const VALIDATOR: &str = "docs/rust-rewrite/a13/source-amendments/validator.rs";
const ARCHIVE_ROOT: &str = "docs/rust-rewrite/a06/source-amendments/real-clock-09ee-v1/";
#[derive(Clone, Copy)]
pub(super) enum Role {
    SharedCurrent,
    ArtifactCurrent,
    FrozenSupport,
    ScopedHistory,
    A13Additional,
    PrivateHttp,
}
fn permitted(role: Role, path: &str) -> bool {
    match role {
        Role::SharedCurrent => path == HTTP || path == VALIDATOR,
        Role::ArtifactCurrent | Role::PrivateHttp => path == HTTP,
        Role::FrozenSupport | Role::ScopedHistory | Role::A13Additional => path == SUPPORT,
    }
}
pub(super) fn selected(path: &str) -> bool {
    [HTTP, SUPPORT, VALIDATOR].contains(&path)
}
fn scope(doc: &Value) -> Result<(), String> {
    need(
        doc["kind"] == "A06_REAL_CLOCK_TRACE_CHILD_V1"
            && doc["source_parent"] == "09ee5bc5f0e1c4f203f7e3df778d74691fef51df"
            && doc["scope"] == json!([HTTP, SUPPORT, VALIDATOR]),
        "real-clock exact child scope",
    )?;
    let rows = doc["transitions"]
        .as_array()
        .ok_or("real-clock transitions missing")?;
    need(
        rows.len() == 3
            && rows
                .iter()
                .map(|r| r["current"]["path"].as_str().unwrap_or(""))
                .eq([HTTP, SUPPORT, VALIDATOR]),
        "real-clock exact transition membership",
    )
}
fn load(root: &Path) -> Result<Value, String> {
    let raw = read(
        root,
        &json!({"path":CAPSULE,"sha256":CAPSULE_SHA,"bytes":CAPSULE_BYTES,"mode":"0644"}),
    )?;
    let doc: Value = serde_json::from_slice(&raw).map_err(|e| e.to_string())?;
    scope(&doc)?;
    for authority in doc["prior_authorities"]
        .as_array()
        .ok_or("real-clock prior authorities missing")?
    {
        read(root, authority)?;
    }
    Ok(doc)
}
fn row<'a>(doc: &'a Value, path: &str) -> Result<&'a Value, String> {
    let rows: Vec<_> = doc["transitions"]
        .as_array()
        .ok_or("real-clock transitions missing")?
        .iter()
        .filter(|r| r["current"]["path"] == path)
        .collect();
    need(rows.len() == 1, "real-clock missing/duplicate child path")?;
    Ok(rows[0])
}
fn old_tuple(expected: &Value, prior: &Value) -> Result<(), String> {
    need(
        ["path", "sha256", "bytes", "mode"]
            .iter()
            .all(|key| expected[*key] == prior[*key])
            && expected
                .get("git_blob")
                .is_none_or(|v| *v == prior["git_blob"]),
        "real-clock exact09ee typed tuple differs",
    )
}
fn archive(root: &Path, child: &Value) -> Result<Vec<u8>, String> {
    let path = text(&child["prior_current"], "path")?;
    need(
        child["archive"]["path"] == format!("{ARCHIVE_ROOT}{path}.frozen")
            && child["archive"]["source_path"] == path
            && child["prior_current"]["path"] == child["current"]["path"],
        "real-clock archive/source linkage",
    )?;
    let before = read(root, &child["archive"])?;
    material(&child["prior_current"], &before, "0644")?;
    Ok(before)
}
fn verify(
    child: &Value,
    before: &[u8],
    actual: &[u8],
    mode: &str,
    edit_sha: &str,
) -> Result<(), String> {
    let path = text(&child["current"], "path")?;
    need(
        selected(path) && child["archive"]["path"] == format!("{ARCHIVE_ROOT}{path}.frozen"),
        "real-clock exact current/archive path scope",
    )?;
    old_tuple(
        &child["prior_current"],
        &json!({"path":child["current"]["path"],"sha256":child["archive"]["sha256"],"bytes":child["archive"]["bytes"],"mode":child["archive"]["mode"],"git_blob":child["prior_current"]["git_blob"]}),
    )?;
    need(
        child["archive"]["source_path"] == child["prior_current"]["path"],
        "real-clock archive source differs",
    )?;
    material(&child["prior_current"], before, "0644")?;
    material(&child["current"], actual, mode)?;
    need(
        digest(&serde_json::to_vec(&child["edits"]).map_err(|e| e.to_string())?) == edit_sha,
        "real-clock sealed child edits differ",
    )?;
    need(
        edits(before, &child["edits"])? == actual,
        "real-clock exact child reconstruction differs",
    )
}
/// Return explicitly archived09ee bytes only. The caller must prove its original
/// transition against them, then call prove_current before accepting any route.
pub(super) fn archived_prior(root: &Path, expected: &Value, role: Role) -> Result<Vec<u8>, String> {
    let path = text(expected, "path")?;
    need(
        permitted(role, path),
        "real-clock typed historical role differs",
    )?;
    let doc = load(root)?;
    let child = row(&doc, path)?;
    old_tuple(expected, &child["prior_current"])?;
    archive(root, child)
}
pub(super) fn prove_current(
    root: &Path,
    expected: &Value,
    role: Role,
    prior: &[u8],
) -> Result<Value, String> {
    let path = text(expected, "path")?;
    need(
        permitted(role, path),
        "real-clock typed current role differs",
    )?;
    let doc = load(root)?;
    let child = row(&doc, path)?;
    old_tuple(expected, &child["prior_current"])?;
    need(
        archive(root, child)? == prior,
        "real-clock supplied09ee archive differs",
    )?;
    let actual = read(root, &child["current"])?;
    verify(
        child,
        prior,
        &actual,
        &permissions(&root.join(path))?,
        text(child, "edits_sha256")?,
    )?;
    Ok(child["current"].clone())
}
pub(super) fn proven_historical(
    root: &Path,
    expected: &Value,
    role: Role,
) -> Result<Vec<u8>, String> {
    let prior = archived_prior(root, expected, role)?;
    prove_current(root, expected, role, &prior)?;
    Ok(prior)
}
pub(super) fn effective_current(
    root: &Path,
    expected: &Value,
    role: Role,
) -> Result<Value, String> {
    let prior = archived_prior(root, expected, role)?;
    prove_current(root, expected, role, &prior)
}
pub(super) fn a13_additional(
    root: &Path,
    manifest: &Value,
    supplied: &[u8],
    mode: &str,
) -> Result<Vec<u8>, String> {
    let doc = load(root)?;
    let child = row(&doc, SUPPORT)?;
    // This live additional-source check is distinct from every historical role.
    let frozen = &doc["prior_authorities"][4];
    let raw = read(root, frozen)?;
    let original: Value = serde_json::from_slice(&raw).map_err(|e| e.to_string())?;
    need(
        *manifest == original,
        "real-clock A13 additional caller manifest differs",
    )?;
    let fields = manifest["additional_sources"]
        .as_array()
        .ok_or("real-clock A13 additional binding absent")?;
    need(fields.len() == 1, "real-clock A13 additional membership")?;
    old_tuple(&fields[0], &child["prior_current"])?;
    let prior = proven_historical(root, &fields[0], Role::A13Additional)?;
    material(&child["current"], supplied, mode)?;
    need(
        read(root, &child["current"])? == supplied,
        "real-clock supplied additional child differs",
    )?;
    Ok(prior)
}
pub(super) fn scoped_http(
    root: &Path,
    successor: &Value,
    historical: &[u8],
) -> Result<Value, String> {
    let doc = load(root)?;
    let child = row(&doc, HTTP)?;
    old_tuple(&successor["current"], &child["prior_current"])?;
    material(&child["scoped_preimage"], historical, "0644")?;
    let effective = effective_current(root, &successor["current"], Role::SharedCurrent)?;
    let actual = read(root, &effective)?;
    need(
        digest(&serde_json::to_vec(&child["scoped_edits"]).map_err(|e| e.to_string())?)
            == text(child, "scoped_edits_sha256")?,
        "real-clock sealed scoped child edits differ",
    )?;
    need(
        edits(historical, &child["scoped_edits"])? == actual,
        "real-clock complete historical191-to-child differs",
    )?;
    let mut next = successor.clone();
    next["current"] = effective;
    next["edits"]=json!(child["scoped_edits"].as_array().ok_or("real-clock scoped edits missing")?.iter().map(|e|Ok(json!({"at":e["at"],"remove":text(e,"remove_utf8")?.as_bytes(),"insert":text(e,"insert_utf8")?.as_bytes()}))).collect::<Result<Vec<Value>,String>>()?);
    Ok(next)
}
pub(super) fn scoped_history(root: &Path, expected: &Value) -> Result<Value, String> {
    proven_historical(root, expected, Role::ScopedHistory)?;
    let doc = load(root)?;
    Ok(row(&doc, SUPPORT)?["archive"].clone())
}
fn private_tuple(
    doc: &Value,
    caller: super::RealClockHttpCaller,
    prior: &Value,
) -> Result<Value, String> {
    let after = match caller {
        super::RealClockHttpCaller::A07 => &doc["transitions"][0]["after"],
        super::RealClockHttpCaller::A08 => &doc["http_transition_reference"]["after"],
    };
    need(
        after["path"] == HTTP && after["mode"] == 420,
        "real-clock private HTTP row role/mode differs",
    )?;
    let expected =
        json!({"path":HTTP,"sha256":after["sha256"],"bytes":after["bytes"],"mode":"0644"});
    old_tuple(&expected, prior)?;
    Ok(expected)
}
pub(super) fn private_http(
    root: &Path,
    caller: super::RealClockHttpCaller,
) -> Result<Vec<u8>, String> {
    let doc = load(root)?;
    let child = row(&doc, HTTP)?;
    let index = match caller {
        super::RealClockHttpCaller::A07 => 5,
        super::RealClockHttpCaller::A08 => 6,
    };
    let frozen: Value = serde_json::from_slice(&read(root, &doc["prior_authorities"][index])?)
        .map_err(|e| e.to_string())?;
    let expected = private_tuple(&frozen, caller, &child["prior_current"])?;
    proven_historical(root, &expected, Role::PrivateHttp)
}

#[cfg(test)]
pub(super) fn controls(root: &Path) -> Result<(), String> {
    let doc = load(root)?;
    for mutation in 0..4 {
        let mut bad = doc.clone();
        match mutation {
            0 => {
                bad["transitions"].as_array_mut().unwrap().pop();
            }
            1 => {
                let first = bad["transitions"][0].clone();
                bad["transitions"].as_array_mut().unwrap().push(first);
            }
            2 => bad["transitions"][0]["current"]["path"] = json!("unlisted.rs"),
            _ => bad["scope"][0] = json!("unlisted.rs"),
        };
        need(
            scope(&bad).is_err(),
            "real-clock missing/duplicate/extra scope accepted",
        )?;
    }
    for child in doc["transitions"]
        .as_array()
        .ok_or("real-clock controls missing")?
    {
        let before = archive(root, child)?;
        let actual = read(root, &child["current"])?;
        let pin = text(child, "edits_sha256")?;
        verify(child, &before, &actual, "0644", pin)?;
        need(
            verify(child, &before, &before, "0644", pin).is_err(),
            "real-clock09ee historical accepted as actual child",
        )?;
        for mode in ["0600", "0755", "4644"] {
            need(
                verify(child, &before, &actual, mode, pin).is_err(),
                "real-clock changed mode accepted",
            )?;
        }
        let mut changed = actual.clone();
        changed.push(0);
        let mut claim = child.clone();
        claim["current"]["sha256"] = json!(digest(&changed));
        claim["current"]["bytes"] = json!(changed.len());
        need(
            verify(&claim, &before, &changed, "0644", pin).is_err(),
            "real-clock rehashed unrelated current accepted",
        )?;
        for mutation in 0..7 {
            let mut bad = child.clone();
            match mutation {
                0 => bad["archive"]["source_path"] = json!("wrong.rs"),
                1 => bad["prior_current"]["path"] = json!("wrong.rs"),
                2 => {
                    bad["edits"].as_array_mut().unwrap().pop();
                }
                3 => {
                    let first = bad["edits"][0].clone();
                    bad["edits"].as_array_mut().unwrap().push(first);
                }
                4 => bad["edits"][0]["at"] = json!(before.len() + 1),
                5 => bad["archive"]["path"] = json!("wrong.rs"),
                _ => bad["edits"][0]["insert_utf8"] = json!("unrelated"),
            };
            let rehashed = digest(&serde_json::to_vec(&bad["edits"]).map_err(|e| e.to_string())?);
            need(
                verify(&bad, &before, &actual, "0644", &rehashed).is_err(),
                "real-clock structurally rehashed edit accepted",
            )?;
        }
    }
    need(
        !permitted(Role::ArtifactCurrent, SUPPORT)
            && !permitted(Role::A13Additional, HTTP)
            && !selected("unlisted.rs"),
        "real-clock wrong typed caller/path selected",
    )?;
    let mut wrong = doc["transitions"][0]["prior_current"].clone();
    wrong["sha256"] = json!("wrong");
    need(
        archived_prior(root, &wrong, Role::SharedCurrent).is_err(),
        "real-clock changed09ee tuple accepted",
    )?;
    let original: Value = serde_json::from_slice(&read(root, &doc["prior_authorities"][4])?)
        .map_err(|e| e.to_string())?;
    let support = row(&doc, SUPPORT)?;
    let support_actual = read(root, &support["current"])?;
    a13_additional(root, &original, &support_actual, "0644")?;
    let mut wrong_manifest = original.clone();
    wrong_manifest["additional_sources"][0]["path"] = json!("unlisted.rs");
    need(
        a13_additional(root, &wrong_manifest, &support_actual, "0644").is_err(),
        "real-clock substituted A13 additional caller accepted",
    )?;
    let mut altered_support = support_actual.clone();
    altered_support.push(0);
    need(
        a13_additional(root, &original, &altered_support, "0644").is_err(),
        "real-clock altered actual support child accepted",
    )?;
    let support_prior = archive(root, support)?;
    need(
        a13_additional(root, &original, &support_prior, "0644").is_err(),
        "real-clock historical support supplied as actual child",
    )?;
    for (caller, index) in [
        (super::RealClockHttpCaller::A07, 5),
        (super::RealClockHttpCaller::A08, 6),
    ] {
        let prior = super::real_clock_http_prior(root, caller)?;
        need(
            prior == archive(root, row(&doc, HTTP)?)?,
            "real-clock private caller returned other historical bytes",
        )?;
        let frozen: Value = serde_json::from_slice(&read(root, &doc["prior_authorities"][index])?)
            .map_err(|e| e.to_string())?;
        private_tuple(&frozen, caller, &row(&doc, HTTP)?["prior_current"])?;
        for mutation in 0..4 {
            let mut bad = frozen.clone();
            let selected = match caller {
                super::RealClockHttpCaller::A07 => &mut bad["transitions"][0]["after"],
                super::RealClockHttpCaller::A08 => &mut bad["http_transition_reference"]["after"],
            };
            match mutation {
                0 => selected["path"] = json!("wrong.rs"),
                1 => selected["mode"] = json!(493),
                2 => selected["sha256"] = json!("wrong"),
                _ => selected["bytes"] = json!(0),
            };
            need(
                private_tuple(&bad, caller, &row(&doc, HTTP)?["prior_current"]).is_err(),
                "real-clock private caller tuple mutation accepted",
            )?;
        }
    }
    eprintln!(
        "A06_REAL_CLOCK_CHILD_CONTROLS exact_three_archived09ee_original_routes_scoped_composition_modes_and_rehashed_edits_rejected=true"
    );
    Ok(())
}
