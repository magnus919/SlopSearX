//! Advisory-only identities for reviewed path forks. Never a Cargo resolver input.
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    io::Read,
    path::{Component, Path, PathBuf},
};
type Result<T> = std::result::Result<T, String>;
const REGISTRY: &str = "registry+https://github.com/rust-lang/crates.io-index";
const MAPPING_PATH: &str = "docs/rust-rewrite/ci/upstream-fork-advisories.json";
const MAPPING: &str = include_str!("../../../docs/rust-rewrite/ci/upstream-fork-advisories.json");
const JSON_CAP: u64 = 32 * 1024 * 1024;
fn need(ok: bool, why: &str) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(format!("upstream advisory view: {why}"))
    }
}
fn hash(raw: &[u8]) -> String {
    Sha256::digest(raw)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn text<'a>(v: &'a Value, k: &str) -> Result<&'a str> {
    v[k].as_str().ok_or_else(|| format!("missing text {k}"))
}
fn read(path: &Path, cap: u64) -> Result<Vec<u8>> {
    let before = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    need(
        before.is_file() && before.len() <= cap,
        "bounded regular file",
    )?;
    let mut raw = Vec::new();
    File::open(path)
        .map_err(|e| e.to_string())?
        .take(cap + 1)
        .read_to_end(&mut raw)
        .map_err(|e| e.to_string())?;
    need(
        raw.len() as u64 == before.len() && raw.len() as u64 <= cap,
        "file changed size or exceeded bound",
    )?;
    Ok(raw)
}
fn source(root: &Path, relative: &str) -> Result<PathBuf> {
    need(
        !relative.is_empty()
            && !relative.contains('\\')
            && Path::new(relative)
                .components()
                .all(|c| matches!(c, Component::Normal(_))),
        "source path escape",
    )?;
    let p = root.join(relative);
    need(
        p.canonicalize()
            .map_err(|e| e.to_string())?
            .starts_with(root),
        "source outside canonical root",
    )?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        need(
            fs::symlink_metadata(&p)
                .map_err(|e| e.to_string())?
                .permissions()
                .mode()
                & 0o7777
                == 0o644,
            "source requires0644",
        )?;
    }
    Ok(p)
}
fn current_tree(root: &Path, relative: &str) -> Result<(usize, String)> {
    let base = root.join(relative);
    let mut pending = vec![base.clone()];
    let mut members = BTreeMap::new();
    let mut total = 0u64;
    while let Some(dir) = pending.pop() {
        for entry in fs::read_dir(dir).map_err(|e| e.to_string())? {
            let p = entry.map_err(|e| e.to_string())?.path();
            let m = fs::symlink_metadata(&p).map_err(|e| e.to_string())?;
            need(!m.file_type().is_symlink(), "fork symlink")?;
            if m.is_dir() {
                pending.push(p);
            } else {
                need(
                    m.is_file() && members.len() < 2048,
                    "fork member kind/count",
                )?;
                let raw = read(&p, 4 * 1024 * 1024)?;
                total = total
                    .checked_add(raw.len() as u64)
                    .ok_or("fork size overflow")?;
                need(total <= 64 * 1024 * 1024, "fork tree bound")?;
                let rel = p
                    .strip_prefix(&base)
                    .map_err(|e| e.to_string())?
                    .to_str()
                    .ok_or("fork UTF8")?
                    .replace('\\', "/");
                need(
                    members.insert(rel, hash(&raw)).is_none(),
                    "duplicate fork member",
                )?;
            }
        }
    }
    let mut records = Vec::new();
    for (path, digest) in &members {
        records.extend_from_slice(path.as_bytes());
        records.push(0);
        records.extend_from_slice(digest.as_bytes());
        records.push(b'\n');
    }
    Ok((members.len(), hash(&records)))
}
fn manifest_identity(row: &Value, raw: &[u8]) -> Result<()> {
    need(
        row["manifest_bytes"].as_u64() == Some(raw.len() as u64)
            && row["manifest_sha256"] == hash(raw),
        "current fork manifest identity",
    )
}
fn provenance_identity(row: &Value, provenance: &Value, raw: &[u8]) -> Result<()> {
    need(
        provenance["sha256"] == hash(raw),
        "reviewed provenance bytes differ",
    )?;
    let doc: Value = serde_json::from_slice(raw).map_err(|e| e.to_string())?;
    let endpoint = doc
        .pointer(text(provenance, "pointer")?)
        .ok_or("upstream provenance pointer")?;
    need(
        endpoint[text(provenance, "version_field")?] == row["version"]
            && endpoint[text(provenance, "checksum_field")?] == row["archive_sha256"],
        "upstream provenance identity differs",
    )
}
fn mapping(root: &Path, supplied: &[u8]) -> Result<Value> {
    need(
        supplied == MAPPING.as_bytes(),
        "mapping differs from compiled reviewed bytes",
    )?;
    let map: Value = serde_json::from_slice(supplied).map_err(|e| e.to_string())?;
    need(
        map["schema"] == 1 && map["forks"].as_array().is_some_and(|x| x.len() == 8),
        "closed eight fork mappings",
    )?;
    for row in map["root_input_identities"]
        .as_array()
        .ok_or("root input identities")?
    {
        need(
            row["sha256"] == hash(&read(&source(root, text(row, "path")?)?, 1024 * 1024)?),
            "physical root build/policy input differs",
        )?;
    }
    for row in map["forks"].as_array().ok_or("fork rows")? {
        let relative = format!("{}/Cargo.toml", text(row, "path")?);
        let raw = read(&source(root, &relative)?, 1024 * 1024)?;
        manifest_identity(row, &raw)?;
        let (count, tree) = current_tree(root, text(row, "path")?)?;
        need(
            row["current_regular_files"].as_u64() == Some(count as u64)
                && row["current_tree_sha256"] == tree,
            "current fork tree differs",
        )?;
        for license in row["licenses"].as_array().ok_or("license identities")? {
            let raw = read(&source(root, text(license, "path")?)?, 1024 * 1024)?;
            need(
                license["sha256"] == hash(&raw) && license["bytes"] == raw.len() as u64,
                "license identity",
            )?;
        }
        for provenance in row["reviewed_provenance"]
            .as_array()
            .ok_or("provenance identities")?
        {
            let raw = read(&source(root, text(provenance, "path")?)?, 1024 * 1024)?;
            provenance_identity(row, provenance, &raw)?;
        }
        if let Some(selected) = row["selected_pristine_identity"].as_array() {
            for identity in selected {
                let raw = read(&source(root, text(identity, "path")?)?, 1024 * 1024)?;
                need(
                    identity["sha256"] == hash(&raw),
                    "selected upstream identity differs",
                )?;
            }
        }
    }
    for row in map["first_party"]
        .as_array()
        .ok_or("first-party vendor classes")?
    {
        need(
            row["sha256"] == hash(&read(&source(root, text(row, "manifest")?)?, 1024 * 1024)?),
            "first-party vendor manifest differs",
        )?;
    }
    Ok(map)
}
fn graph(metadata: &Value) -> Result<BTreeSet<String>> {
    need(metadata["version"] == 1, "Cargo metadata version")?;
    let packages = metadata["packages"]
        .as_array()
        .ok_or("full metadata packages")?;
    need(
        !packages.is_empty() && packages.len() <= 4096,
        "package count bound",
    )?;
    let mut ids = BTreeSet::new();
    for p in packages {
        need(
            ids.insert(text(p, "id")?.to_owned()),
            "duplicate package ID",
        )?;
        need(
            p.get("source")
                .is_some_and(|x| x.is_null() || x.is_string()),
            "invalid package source field",
        )?;
    }
    let nodes = metadata["resolve"]["nodes"]
        .as_array()
        .ok_or("full resolve required; no-deps metadata rejected")?;
    need(nodes.len() <= 4096, "resolve node count bound")?;
    let mut graph_ids = BTreeSet::new();
    for n in nodes {
        need(
            ids.contains(text(n, "id")?) && graph_ids.insert(text(n, "id")?.to_owned()),
            "unknown or duplicate resolve node",
        )?;
    }
    for n in nodes {
        let deps = n["deps"].as_array().ok_or("resolve deps")?;
        let legacy = n["dependencies"]
            .as_array()
            .ok_or("legacy resolve dependencies")?;
        need(
            deps.len() <= 4096 && legacy.len() <= 4096,
            "dependency edge bound",
        )?;
        for dep in deps {
            need(
                graph_ids.contains(text(dep, "pkg")?),
                "dependency outside resolved nodes",
            )?;
        }
        for dep in legacy {
            need(
                dep.as_str().is_some_and(|id| graph_ids.contains(id)),
                "legacy dependency outside resolved nodes",
            )?;
        }
    }
    for field in ["workspace_members", "workspace_default_members"] {
        let roots = metadata[field].as_array().ok_or("workspace roots")?;
        let mut unique = BTreeSet::new();
        for id in roots {
            let id = id.as_str().ok_or("workspace root ID")?;
            need(
                graph_ids.contains(id) && unique.insert(id),
                "unknown or duplicate workspace root",
            )?;
        }
    }
    match metadata["resolve"]
        .get("root")
        .ok_or("missing resolve root field")?
    {
        Value::Null => {}
        Value::String(id) => need(graph_ids.contains(id), "unknown resolve root")?,
        _ => return Err("upstream advisory view: malformed resolve root".into()),
    }
    Ok(graph_ids)
}
fn derive(root: &Path, raw: &Value, map: &Value) -> Result<(Value, Value)> {
    let nodes = graph(raw)?;
    need(
        Path::new(text(raw, "workspace_root")?)
            .canonicalize()
            .map_err(|e| e.to_string())?
            == root,
        "metadata workspace caller differs",
    )?;
    let packages = raw["packages"].as_array().ok_or("packages")?;
    let rows = map["forks"].as_array().ok_or("mappings")?;
    let mut selected = BTreeMap::new();
    let mut seen = BTreeSet::new();
    let mut expected_members = BTreeSet::new();
    let first = map["first_party"].as_array().ok_or("first-party")?;
    // Establish every required workspace origin from the compiled map, before
    // looking at the supplied workspace root lists. Removing a package/node and
    // its roots must not shrink the expected set.
    for expected in rows
        .iter()
        .chain(first)
        .filter(|x| x["workspace_member"] == true)
    {
        let wanted = if let Some(path) = expected["path"].as_str() {
            root.join(path).join("Cargo.toml")
        } else {
            root.join(text(expected, "manifest")?)
        };
        let found = packages
            .iter()
            .filter(|p| {
                p["manifest_path"]
                    .as_str()
                    .and_then(|path| Path::new(path).canonicalize().ok())
                    .as_ref()
                    == Some(&wanted)
            })
            .collect::<Vec<_>>();
        need(
            found.len() == 1,
            "reviewed workspace member missing or duplicate",
        )?;
        let name = expected
            .get("current_package_name")
            .or_else(|| expected.get("name"))
            .ok_or("workspace class name")?;
        need(
            found[0]["name"] == *name
                && found[0]["version"] == expected["version"]
                && found[0]["source"].is_null(),
            "reviewed workspace member identity differs",
        )?;
        need(
            expected_members.insert(text(found[0], "id")?.to_owned()),
            "duplicate reviewed workspace class",
        )?;
    }
    for (index, package) in packages.iter().enumerate() {
        let manifest = Path::new(text(package, "manifest_path")?)
            .canonicalize()
            .map_err(|e| e.to_string())?;
        let row = rows.iter().find(|r| {
            root.join(format!("{}/Cargo.toml", r["path"].as_str().unwrap_or(""))) == manifest
        });
        if let Some(row) = row {
            need(package["source"].is_null(), "mapped package source differs")?;
            need(
                package["name"] == row["current_package_name"],
                "mapped package name differs",
            )?;
            need(
                package["version"] == row["version"],
                "mapped package version differs",
            )?;
            need(
                text(package, "id")?.starts_with("path+file:")
                    && nodes.contains(text(package, "id")?),
                "mapped fork is not active path package",
            )?;
            need(
                seen.insert(text(row, "path")?.to_owned()),
                "duplicate mapped manifest",
            )?;
            selected.insert(index, row);
        } else if package["source"].is_null() {
            let owned = first
                .iter()
                .find(|x| root.join(x["manifest"].as_str().unwrap_or("")) == manifest);
            need(
                owned.is_some_and(|x| {
                    package["name"] == x["name"] && package["version"] == x["version"]
                }),
                "unmapped local package",
            )?;
        }
    }
    need(
        selected.len() == 8 && seen.len() == rows.len(),
        "missing reviewed fork mapping",
    )?;
    for field in ["workspace_members", "workspace_default_members"] {
        let actual = raw[field]
            .as_array()
            .ok_or("workspace roots")?
            .iter()
            .map(|x| {
                x.as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| "workspace root ID".to_owned())
            })
            .collect::<Result<BTreeSet<_>>>()?;
        need(
            actual == expected_members,
            "reviewed workspace membership differs",
        )?;
    }
    let mut view = raw.clone();
    let mut changes = Vec::new();
    for (index, row) in selected {
        view["packages"][index]["source"] = json!(REGISTRY);
        view["packages"][index]["name"] = row["upstream_name"].clone();
        changes.push(json!({"package_id":raw["packages"][index]["id"],"package_index":index,"manifest_path":raw["packages"][index]["manifest_path"],"original_name":row["current_package_name"],"upstream_name":row["upstream_name"],"version":row["version"],"archive_sha256":row["archive_sha256"],"current_manifest_sha256":row["manifest_sha256"]}));
    }
    let mut restored = view.clone();
    for row in &changes {
        let i = row["package_index"].as_u64().ok_or("mapped index")? as usize;
        restored["packages"][i]["source"] = raw["packages"][i]["source"].clone();
        restored["packages"][i]["name"] = raw["packages"][i]["name"].clone();
    }
    need(restored == *raw, "unrelated metadata field changed")?;
    Ok((
        view,
        json!({"scope":"advisory-only; not Cargo resolution, shipping, source, feature, TLS or license evidence","mapped_packages":changes,"only_changed_fields":["packages[i].name","packages[i].source"],"original_package_ids_and_all_other_fields_preserved":true}),
    ))
}
fn save(path: &Path, value: &Value) -> Result<()> {
    let raw = serde_json::to_vec(value).map_err(|e| e.to_string())?;
    need(raw.len() as u64 <= JSON_CAP, "output JSON bound")?;
    fs::write(path, raw).map_err(|e| e.to_string())
}
fn context(raw_path: &Path) -> Result<(PathBuf, Value, Value, Vec<u8>)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let map_raw = read(&source(&root, MAPPING_PATH)?, 1024 * 1024)?;
    let map = mapping(&root, &map_raw)?;
    let raw = read(raw_path, JSON_CAP)?;
    let metadata = serde_json::from_slice(&raw).map_err(|e| e.to_string())?;
    Ok((root, map, metadata, raw))
}
fn controls(root: &Path, raw: &Value, map: &Value) -> Result<Vec<Value>> {
    let (_, positive) = derive(root, raw, map)?;
    let mapped = positive["mapped_packages"][0]["package_index"]
        .as_u64()
        .ok_or("positive index")? as usize;
    let mut receipts = Vec::new();
    for (field, replacement, reason) in [
        (
            "source",
            json!(REGISTRY),
            "upstream advisory view: mapped package source differs",
        ),
        (
            "name",
            json!("unreviewed-fork"),
            "upstream advisory view: mapped package name differs",
        ),
        (
            "version",
            json!("0.0.0"),
            "upstream advisory view: mapped package version differs",
        ),
    ] {
        let mut mutant = raw.clone();
        mutant["packages"][mapped][field] = replacement;
        let rejection = derive(root, &mutant, map).err();
        need(
            rejection.as_deref() == Some(reason),
            "named identity rejection differs",
        )?;
        receipts.push(json!({"field":field,"mutant_sha256":hash(&serde_json::to_vec(&mutant).map_err(|e|e.to_string())?),"rejection":rejection}));
    }
    let mut mutant = raw.clone();
    mutant["resolve"] = Value::Null;
    need(
        derive(root, &mutant, map).is_err(),
        "no-deps input accepted",
    )?;
    let mut mutant = raw.clone();
    let duplicate = mutant["packages"][mapped].clone();
    mutant["packages"]
        .as_array_mut()
        .ok_or("package list")?
        .push(duplicate);
    need(
        derive(root, &mutant, map).is_err(),
        "duplicate package ID accepted",
    )?;
    let mut mutant = map.clone();
    mutant["forks"].as_array_mut().ok_or("mapping rows")?.pop();
    need(
        derive(root, raw, &mutant).is_err(),
        "missing mapping accepted",
    )?;
    let mut extra = map.clone();
    let row = extra["forks"][0].clone();
    extra["forks"]
        .as_array_mut()
        .ok_or("mapping rows")?
        .push(row);
    need(derive(root, raw, &extra).is_err(), "extra mapping accepted")?;
    let mut unknown = raw.clone();
    let mut package = unknown["packages"][mapped].clone();
    package["id"] = json!("path+file:///owned-malformed-fixture#unknown@0.0.0");
    package["manifest_path"] = json!(root.join("Cargo.toml").to_str().ok_or("root UTF8")?);
    unknown["packages"]
        .as_array_mut()
        .ok_or("packages")?
        .push(package);
    need(
        derive(root, &unknown, map).err().as_deref()
            == Some("upstream advisory view: unmapped local package"),
        "unknown local package accepted",
    )?;
    let mut wrong_root = raw.clone();
    wrong_root["workspace_root"] = json!(
        root.parent()
            .ok_or("workspace parent")?
            .to_str()
            .ok_or("root UTF8")?
    );
    need(
        derive(root, &wrong_root, map).err().as_deref()
            == Some("upstream advisory view: metadata workspace caller differs"),
        "wrong workspace caller accepted",
    )?;
    let mut changed = MAPPING.as_bytes().to_vec();
    changed[0] ^= 1;
    need(
        mapping(root, &changed).is_err(),
        "changed mapping bytes accepted",
    )?;
    let nodes = raw["resolve"]["nodes"].as_array().ok_or("actual nodes")?;
    let edge_node = nodes
        .iter()
        .position(|n| {
            n["deps"].as_array().is_some_and(|x| !x.is_empty())
                && n["dependencies"].as_array().is_some_and(|x| !x.is_empty())
        })
        .ok_or("nonvacuous actual dependency edge")?;
    let mut extra_package = raw["packages"][mapped].clone();
    extra_package["id"] = json!("path+file:///owned-malformed-fixture#unresolved-package@0.0.0");
    for (field, reason) in [
        (
            "deps",
            "upstream advisory view: dependency outside resolved nodes",
        ),
        (
            "dependencies",
            "upstream advisory view: legacy dependency outside resolved nodes",
        ),
    ] {
        let mut mutant = raw.clone();
        mutant["packages"]
            .as_array_mut()
            .ok_or("packages")?
            .push(extra_package.clone());
        if field == "deps" {
            mutant["resolve"]["nodes"][edge_node][field][0]["pkg"] = extra_package["id"].clone();
        } else {
            mutant["resolve"]["nodes"][edge_node][field][0] = extra_package["id"].clone();
        }
        let rejection = derive(root, &mutant, map).err();
        need(
            rejection.as_deref() == Some(reason),
            "named malformed-edge rejection differs",
        )?;
        receipts.push(json!({"field":field,"changes":["append package ID absent from resolve nodes","point actual edge to that ID"],"rejection":rejection}));
    }
    for field in ["workspace_members", "workspace_default_members"] {
        let mut mutant = raw.clone();
        let id = mutant[field][0].clone();
        need(id.is_string(), "nonvacuous actual workspace root")?;
        mutant[field].as_array_mut().ok_or("roots")?.push(id);
        need(
            derive(root, &mutant, map).err().as_deref()
                == Some("upstream advisory view: unknown or duplicate workspace root"),
            "duplicate actual workspace root accepted",
        )?;
    }
    let mut mutant = raw.clone();
    mutant["resolve"]["root"] = json!(42);
    need(
        derive(root, &mutant, map).err().as_deref()
            == Some("upstream advisory view: malformed resolve root"),
        "malformed resolve root accepted",
    )?;
    let leaf = raw["packages"]
        .as_array()
        .ok_or("actual packages")?
        .iter()
        .find(|p| p["name"] == "ssx-server")
        .ok_or("actual first-party leaf")?;
    let leaf_id = text(leaf, "id")?;
    need(
        nodes.iter().all(|n| {
            n["deps"]
                .as_array()
                .is_some_and(|deps| deps.iter().all(|d| d["pkg"] != leaf_id))
                && n["dependencies"]
                    .as_array()
                    .is_some_and(|deps| deps.iter().all(|d| d != leaf_id))
        }),
        "nonvacuous actual first-party leaf has incoming edge",
    )?;
    let mut missing_leaf = raw.clone();
    missing_leaf["packages"]
        .as_array_mut()
        .ok_or("packages")?
        .retain(|p| p["id"] != leaf_id);
    missing_leaf["resolve"]["nodes"]
        .as_array_mut()
        .ok_or("nodes")?
        .retain(|n| n["id"] != leaf_id);
    for field in ["workspace_members", "workspace_default_members"] {
        missing_leaf[field]
            .as_array_mut()
            .ok_or("roots")?
            .retain(|id| id != leaf_id);
    }
    graph(&missing_leaf)?;
    need(
        derive(root, &missing_leaf, map).err().as_deref()
            == Some("upstream advisory view: reviewed workspace member missing or duplicate"),
        "removed real first-party leaf class accepted",
    )?;
    receipts.push(json!({"control":"remove actual first-party leaf package/node/roots together","removed_actual_package_id":leaf_id,"malformed_graph_is_not_rejection_reason":true,"rejection":"reviewed workspace member missing or duplicate"}));
    let first = &map["forks"][0];
    let manifest = read(
        &source(root, &format!("{}/Cargo.toml", text(first, "path")?))?,
        1024 * 1024,
    )?;
    manifest_identity(first, &manifest)?;
    let mut changed = manifest.clone();
    changed[0] ^= 1;
    for raw in [&changed[..], &[][..]] {
        need(
            manifest_identity(first, raw).err().as_deref()
                == Some("upstream advisory view: current fork manifest identity"),
            "changed/missing manifest bytes accepted",
        )?;
    }
    let provenance = &first["reviewed_provenance"][0];
    let material = read(&source(root, text(provenance, "path")?)?, 1024 * 1024)?;
    provenance_identity(first, provenance, &material)?;
    let mut changed = material.clone();
    changed[0] ^= 1;
    for raw in [&changed[..], &[][..]] {
        need(
            provenance_identity(first, provenance, raw).err().as_deref()
                == Some("upstream advisory view: reviewed provenance bytes differ"),
            "changed/missing provenance bytes accepted",
        )?;
    }
    receipts.push(json!({"additional_rejected":["no_deps","duplicate_id","missing_mapping","changed_mapping_bytes","duplicate_workspace_roots","malformed_resolve_root","changed_or_missing_manifest_material","changed_or_missing_provenance_material"],"actual_positive_before_mutants":true}));
    Ok(receipts)
}

fn advisory_ids(map: &Value) -> Result<BTreeMap<String, String>> {
    let names = map["forks"]
        .as_array()
        .ok_or("fork rows")?
        .iter()
        .map(|x| text(x, "upstream_name").map(str::to_owned))
        .collect::<Result<BTreeSet<_>>>()?;
    Ok(names
        .into_iter()
        .enumerate()
        .map(|(i, name)| (name, format!("RUSTSEC-2026-9{:03}", i + 1)))
        .collect())
}
fn expected_diagnostics(metadata: &Value, ids: &BTreeMap<String, String>) -> Result<Value> {
    let nodes = graph(metadata)?;
    let mut counts = BTreeMap::<String, (String, String, String, usize)>::new();
    for p in metadata["packages"].as_array().ok_or("packages")? {
        if p["source"] != REGISTRY || !nodes.contains(text(p, "id")?) {
            continue;
        }
        let name = text(p, "name")?;
        if let Some(id) = ids.get(name) {
            let version = text(p, "version")?;
            let key = format!("{id}|{name}|{version}");
            counts
                .entry(key)
                .or_insert((id.clone(), name.to_owned(), version.to_owned(), 0))
                .3 += 1;
        }
    }
    Ok(json!(counts.values().map(|(id,name,version,count)|json!({"advisory_id":id,"package":name,"version":version,"count":count})).collect::<Vec<_>>()))
}
fn full_fixture_expectations(
    raw: &Value,
    map: &Value,
    ids: &BTreeMap<String, String>,
) -> Result<Value> {
    // Independent oracle: preserve actual registry baseline diagnostics, then
    // require one additional match for EACH of the eight reviewed fork origins.
    // Do not derive expected success from whatever identities the mapper emits.
    let baseline = expected_diagnostics(raw, ids)?;
    let mut rows = BTreeMap::<String, Value>::new();
    for row in baseline.as_array().ok_or("baseline diagnostics")? {
        rows.insert(
            format!(
                "{}|{}|{}",
                text(row, "advisory_id")?,
                text(row, "package")?,
                text(row, "version")?
            ),
            row.clone(),
        );
    }
    for fork in map["forks"].as_array().ok_or("fork rows")? {
        let name = text(fork, "upstream_name")?;
        let version = text(fork, "version")?;
        let id = ids.get(name).ok_or("fixture advisory ID")?;
        let key = format!("{id}|{name}|{version}");
        let row = rows.entry(key).or_insert_with(
            || json!({"advisory_id":id,"package":name,"version":version,"count":0}),
        );
        row["count"] = json!(row["count"].as_u64().ok_or("diagnostic count")? + 1);
    }
    Ok(json!(rows.into_values().collect::<Vec<_>>()))
}
fn fixture_documents(
    raw: &Value,
    view: &Value,
    identity: &Value,
    map: &Value,
) -> Result<(BTreeMap<String, Value>, Value)> {
    let mut variants = BTreeMap::from([
        ("raw.json".to_owned(), raw.clone()),
        ("view.json".to_owned(), view.clone()),
    ]);
    let hyper = identity["mapped_packages"]
        .as_array()
        .ok_or("mapped instances")?
        .iter()
        .filter(|x| x["upstream_name"] == "hyper")
        .collect::<Vec<_>>();
    need(
        hyper.len() == 2,
        "two distinct mapped Hyper instances required",
    )?;
    let mut restored_ids = Vec::new();
    for (n, row) in hyper.iter().enumerate() {
        let index = row["package_index"].as_u64().ok_or("mapped index")? as usize;
        let mut variant = view.clone();
        variant["packages"][index]["name"] = raw["packages"][index]["name"].clone();
        variant["packages"][index]["source"] = raw["packages"][index]["source"].clone();
        variants.insert(format!("hyper-{n}-restored.json"), variant);
        restored_ids.push(row["package_id"].clone());
    }
    need(
        variants["hyper-0-restored.json"] != variants["hyper-1-restored.json"],
        "distinct Hyper restoration variants required",
    )?;
    let ids = advisory_ids(map)?;
    let full_expected = full_fixture_expectations(raw, map, &ids)?;
    let run_names = [
        ("raw-omission", "vulnerable.toml", "raw.json"),
        ("mapped-detection", "vulnerable.toml", "view.json"),
        ("safe-view", "safe.toml", "view.json"),
        (
            "hyper-0-restored",
            "vulnerable.toml",
            "hyper-0-restored.json",
        ),
        (
            "hyper-1-restored",
            "vulnerable.toml",
            "hyper-1-restored.json",
        ),
    ];
    let mut runs = Vec::new();
    for (name, config, file) in run_names {
        let metadata = &variants[file];
        let expected = if name == "raw-omission" {
            expected_diagnostics(raw, &ids)?
        } else if config == "safe.toml" {
            json!([])
        } else {
            let mut expected = full_expected.clone();
            if name.starts_with("hyper-") {
                let target = expected
                    .as_array_mut()
                    .ok_or("expected diagnostics")?
                    .iter_mut()
                    .find(|x| x["package"] == "hyper" && x["version"] == "1.12.0")
                    .ok_or("expected Hyper instances")?;
                let count = target["count"].as_u64().ok_or("Hyper count")?;
                need(count >= 2, "nonvacuous two Hyper oracle")?;
                target["count"] = json!(count - 1);
            }
            expected
        };
        let count = expected
            .as_array()
            .ok_or("diagnostics")?
            .iter()
            .map(|x| x["count"].as_u64().unwrap_or(0))
            .sum::<u64>();
        runs.push(json!({"name":name,"config":config,"metadata":file,"metadata_serialized_sha256":hash(&serde_json::to_vec(metadata).map_err(|e|e.to_string())?),"expected_exit":if count==0{0}else{1},"expected_diagnostics":expected}));
    }
    Ok((
        variants,
        json!({"schema":1,"scope":"Owned synthetic advisory DBs, not real vulnerability or fresh production advisory coverage","mapping_sha256":hash(MAPPING.as_bytes()),"advisory_ids":ids,"restored_hyper_original_ids":restored_ids,"runs":runs}),
    ))
}
fn advisory_text(name: &str, id: &str, safe: bool) -> String {
    let patched = if safe { ">=0.0.0" } else { ">=999.0.0" };
    format!(
        "```toml\n[advisory]\nid = \"{id}\"\npackage = \"{name}\"\ndate = \"2026-10-31\"\nurl = \"https://example.invalid/owned-advisory-fixture\"\n[versions]\npatched = [\"{patched}\"]\n```\n\n# Owned synthetic advisory fixture\n\nThis is qualification data, not a report of a real vulnerability.\n"
    )
}
fn fixture_config(out: &Path, kind: &str) -> Result<String> {
    let source = out.join(format!("{kind}-source"));
    // cargo-deny requires a domain; url normalizes bare localhost away.
    // Unix Git file transport uses this absolute path locally, without DNS.
    let url = format!(
        "file://localhost.{}",
        source.to_str().ok_or("fixture UTF8")?
    );
    // JSON quoted strings are compatible with the restricted strings used here.
    Ok(format!(
        "[advisories]\ndb-path = {}\ndb-urls = [{}]\nyanked = \"allow\"\n",
        json!(out.join(format!("db-{kind}")).to_str().ok_or("db UTF8")?),
        json!(url)
    ))
}
fn prepare_fixtures(raw_path: &Path, out: &Path) -> Result<()> {
    let (root, map, raw, original) = context(raw_path)?;
    let (view, identity) = derive(&root, &raw, &map)?;
    controls(&root, &raw, &map)?;
    let (variants, plan) = fixture_documents(&raw, &view, &identity, &map)?;
    fs::create_dir(out).map_err(|e| e.to_string())?;
    let out = out.canonicalize().map_err(|e| e.to_string())?;
    for (name, value) in variants {
        if name == "raw.json" {
            fs::write(out.join(name), &original).map_err(|e| e.to_string())?;
        } else {
            save(&out.join(name), &value)?;
        }
    }
    save(&out.join("fixture-plan.json"), &plan)?;
    let ids = advisory_ids(&map)?;
    for (kind, safe) in [("safe", true), ("vulnerable", false)] {
        for (name, id) in &ids {
            let dir = out.join(format!("{kind}-source/crates/{name}"));
            fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
            fs::write(dir.join(format!("{id}.md")), advisory_text(name, id, safe))
                .map_err(|e| e.to_string())?;
        }
        fs::write(
            out.join(format!("{kind}.toml")),
            fixture_config(&out, kind)?,
        )
        .map_err(|e| e.to_string())?;
    }
    fs::write(
        out.join("fixture-dbs.tsv"),
        "safe\tsafe-source\tsafe.toml\nvulnerable\tvulnerable-source\tvulnerable.toml\n",
    )
    .map_err(|e| e.to_string())?;
    let runs = plan["runs"]
        .as_array()
        .ok_or("fixture runs")?
        .iter()
        .map(|x| {
            Ok(format!(
                "{}\t{}\t{}\n",
                text(x, "name")?,
                text(x, "config")?,
                text(x, "metadata")?
            ))
        })
        .collect::<Result<String>>()?;
    fs::write(out.join("fixture-runs.tsv"), runs).map_err(|e| e.to_string())?;
    Ok(())
}
fn diagnostics(raw: &[u8]) -> Result<Value> {
    let output = std::str::from_utf8(raw).map_err(|e| e.to_string())?;
    let mut counts = BTreeMap::<String, (String, String, String, usize)>::new();
    for line in output.lines().filter(|x| !x.trim().is_empty()) {
        let row: Value =
            serde_json::from_str(line).map_err(|e| format!("scanner non-JSON output: {e}"))?;
        if row["type"] != "diagnostic" {
            for level in [&row["level"], &row["fields"]["level"]] {
                need(
                    !level
                        .as_str()
                        .is_some_and(|value| value.eq_ignore_ascii_case("error")),
                    "scanner infrastructure error",
                )?;
            }
            continue;
        }
        let fields = &row["fields"];
        need(
            fields["code"] == "vulnerability" && fields["severity"] == "error",
            "unexpected scanner diagnostic",
        )?;
        let notes = fields["notes"].as_array().ok_or("scanner advisory notes")?;
        let ids = notes
            .iter()
            .filter_map(Value::as_str)
            .filter_map(|x| x.strip_prefix("ID: "))
            .collect::<Vec<_>>();
        need(ids.len() == 1, "scanner exact advisory ID")?;
        let labels = fields["labels"].as_array().ok_or("scanner labels")?;
        need(labels.len() == 1, "scanner exact package label")?;
        let span = text(&labels[0], "span")?
            .split_whitespace()
            .collect::<Vec<_>>();
        need(span.len() >= 3, "scanner package/version/source span")?;
        let key = format!("{}|{}|{}", ids[0], span[0], span[1]);
        counts
            .entry(key)
            .or_insert((ids[0].to_owned(), span[0].to_owned(), span[1].to_owned(), 0))
            .3 += 1;
    }
    Ok(json!(counts.values().map(|(id,name,version,count)|json!({"advisory_id":id,"package":name,"version":version,"count":count})).collect::<Vec<_>>()))
}
fn verify_fixtures(out: &Path) -> Result<()> {
    let out = out.canonicalize().map_err(|e| e.to_string())?;
    let (root, map, raw, original) = context(&out.join("raw.json"))?;
    let (view, identity) = derive(&root, &raw, &map)?;
    let (variants, expected_plan) = fixture_documents(&raw, &view, &identity, &map)?;
    let actual_plan: Value =
        serde_json::from_slice(&read(&out.join("fixture-plan.json"), 1024 * 1024)?)
            .map_err(|e| e.to_string())?;
    need(
        actual_plan == expected_plan,
        "fixture plan differs from actual source-bound metadata",
    )?;
    for (name, expected) in variants {
        let actual: Value =
            serde_json::from_slice(&read(&out.join(name), JSON_CAP)?).map_err(|e| e.to_string())?;
        need(actual == expected, "fixture metadata differs")?;
    }
    for (kind, safe) in [("safe", true), ("vulnerable", false)] {
        need(
            read(&out.join(format!("{kind}.toml")), 1024 * 1024)?
                == fixture_config(&out, kind)?.as_bytes(),
            "fixture configuration differs",
        )?;
        let mut actual = BTreeSet::new();
        for entry in
            fs::read_dir(out.join(format!("{kind}-source/crates"))).map_err(|e| e.to_string())?
        {
            let entry = entry.map_err(|e| e.to_string())?;
            for p in fs::read_dir(entry.path()).map_err(|e| e.to_string())? {
                actual.insert(p.map_err(|e| e.to_string())?.path());
            }
        }
        let mut expected = BTreeSet::new();
        for (name, id) in advisory_ids(&map)? {
            let p = out.join(format!("{kind}-source/crates/{name}/{id}.md"));
            need(
                read(&p, 1024 * 1024)? == advisory_text(&name, &id, safe).as_bytes(),
                "synthetic advisory bytes differ",
            )?;
            expected.insert(p);
        }
        need(actual == expected, "synthetic advisory membership differs")?;
    }
    let version = read(&out.join("cargo-deny-version.stdout"), 4096)?;
    let words = std::str::from_utf8(&version)
        .map_err(|e| e.to_string())?
        .split_whitespace()
        .collect::<Vec<_>>();
    need(
        words.first() == Some(&"cargo-deny") && words.get(1) == Some(&"0.20.2"),
        "pinned scanner version",
    )?;
    need(diagnostics(br#"{"type":"log","fields":{"level":"ERROR","message":"owned parser negative, not an actual scanner result"}}"#).is_err(),"scanner nested infrastructure error accepted")?;
    need(
        diagnostics(b"not JSON").is_err(),
        "non-JSON scanner output accepted",
    )?;
    let mut results = Vec::new();
    for run in expected_plan["runs"].as_array().ok_or("runs")? {
        let name = text(run, "name")?;
        let code = read(&out.join(format!("{name}.exit")), 128)?;
        let code = std::str::from_utf8(&code)
            .map_err(|e| e.to_string())?
            .trim()
            .parse::<i32>()
            .map_err(|e| e.to_string())?;
        let stdout = read(&out.join(format!("{name}.stdout")), 8 * 1024 * 1024)?;
        let stderr = read(&out.join(format!("{name}.stderr")), 8 * 1024 * 1024)?;
        let mut combined = stdout.clone();
        combined.push(b'\n');
        combined.extend_from_slice(&stderr);
        let actual = diagnostics(&combined)?;
        need(
            json!(code) == run["expected_exit"] && actual == run["expected_diagnostics"],
            "actual scanner fixture exit/diagnostics differ",
        )?;
        results.push(json!({"name":name,"actual_exit":code,"actual_diagnostics":actual,"stdout_sha256":hash(&stdout),"stderr_sha256":hash(&stderr)}));
    }
    save(
        &out.join("fixture-acceptance.json"),
        &json!({"scope":"real pinned scanner with owned synthetic local Git DBs; not a fresh production advisory scan","scanner_version":std::str::from_utf8(&version).map_err(|e|e.to_string())?.trim(),"raw_current_metadata_sha256":hash(&original),"raw_serialized_metadata_sha256":hash(&serde_json::to_vec(&raw).map_err(|e|e.to_string())?),"mapping_sha256":hash(MAPPING.as_bytes()),"original_hyper_ids_individually_restored":expected_plan["restored_hyper_original_ids"],"results":results}),
    )
}
fn main() -> Result<()> {
    let args = std::env::args().collect::<Vec<_>>();
    if args.len() == 3 && args[1] == "verify-fixtures" {
        return verify_fixtures(Path::new(&args[2]));
    }
    need(
        args.len() == 4,
        "usage: upstream_advisory_view {derive|fixtures} ACTUAL_METADATA_JSON OWNED_OUTPUT_DIRECTORY; or verify-fixtures FIXTURE_DIRECTORY",
    )?;
    if args[1] == "fixtures" {
        return prepare_fixtures(Path::new(&args[2]), Path::new(&args[3]));
    }
    need(args[1] == "derive", "unknown finite operation")?;
    let (root, map, raw, original) = context(Path::new(&args[2]))?;
    let (view, identity) = derive(&root, &raw, &map)?;
    let negative = controls(&root, &raw, &map)?;
    let output = Path::new(&args[3]);
    fs::create_dir(output).map_err(|e| e.to_string())?;
    save(&output.join("upstream-advisory-metadata.json"), &view)?;
    save(
        &output.join("identity-receipt.json"),
        &json!({"raw_metadata_sha256":hash(&original),"raw_serialized_sha256":hash(&serde_json::to_vec(&raw).map_err(|e|e.to_string())?),"derived_serialized_sha256":hash(&serde_json::to_vec(&view).map_err(|e|e.to_string())?),"compiled_mapping_sha256":hash(MAPPING.as_bytes()),"identity":identity,"negative_controls":negative,"scanner_execution":"not performed by derivation; separately required"}),
    )
}
