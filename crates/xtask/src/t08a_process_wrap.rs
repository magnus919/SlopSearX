//! Private developer-owner custody and explicit qualification, never shipping authority.
use crate::{Result, command};
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    fs::{self, File},
    io::Read,
    path::{Component, Path},
};

const VENDOR: &str = "vendor/ssx-cli-process-wrap";
const PROVENANCE: &str = "docs/rust-rewrite/t08a/process-wrap-provenance.json";
const ARCHIVE: &str = "docs/rust-rewrite/t08a/process-wrap-10.0.1.crate";
const PATCH: &str = "docs/rust-rewrite/t08a/process-wrap.patch";
const CHECKSUM: &str = "1f21b97672d2dc848e7b25701ab4535618b92f4861c13cc3f7f7bed52ad3c8da";
const FEATURES: &str = "tokio1,process-group,job-object,kill-on-drop";
const FILE_CAP: u64 = 1_048_576;
fn need(ok: bool, message: &str) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(format!("T08A private process-wrap: {message}"))
    }
}
fn relative(path: &str) -> bool {
    !path.is_empty()
        && !path.contains('\\')
        && Path::new(path)
            .components()
            .all(|c| matches!(c, Component::Normal(_)))
}
fn read(root: &Path, path: &str, expected: Option<u64>) -> Result<Vec<u8>> {
    need(relative(path), "path escape")?;
    let path = root.join(path);
    let metadata = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
    need(
        metadata.is_file() && metadata.len() <= FILE_CAP,
        "regular bounded source",
    )?;
    need(
        path.canonicalize()
            .map_err(|e| e.to_string())?
            .starts_with(root.canonicalize().map_err(|e| e.to_string())?),
        "source escaped root",
    )?;
    let mut bytes = Vec::new();
    File::open(path)
        .map_err(|e| e.to_string())?
        .take(FILE_CAP + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    need(
        bytes.len() as u64 <= FILE_CAP && bytes.len() as u64 == metadata.len(),
        "source grew or changed size during read",
    )?;
    if let Some(expected) = expected {
        need(bytes.len() as u64 == expected, "source length changed")?;
    }
    Ok(bytes)
}
pub(super) fn sources(root: &Path) -> Result<()> {
    let provenance: Value =
        serde_json::from_slice(&read(root, PROVENANCE, None)?).map_err(|e| e.to_string())?;
    need(
        provenance["schema"] == 1
            && provenance["upstream"]["name"] == "process-wrap"
            && provenance["upstream"]["version"] == "10.0.1"
            && provenance["upstream"]["archive_sha256"] == CHECKSUM
            && provenance["private_root"] == VENDOR,
        "upstream identity",
    )?;
    need(
        super::hash(&read(root, ARCHIVE, Some(201_062))?) == CHECKSUM,
        "pristine archive changed",
    )?;
    need(
        provenance["upstream"]["archive"] == ARCHIVE
            && provenance["patch"]["path"] == PATCH
            && provenance["patch"]["apply"] == "git apply --unidiff-zero",
        "custody paths",
    )?;
    let patch_len = provenance["patch"]["bytes"]
        .as_u64()
        .ok_or("private patch length")?;
    need(
        super::hash(&read(root, PATCH, Some(patch_len))?) == provenance["patch"]["sha256"],
        "private patch changed",
    )?;
    let rows = provenance["files"]
        .as_array()
        .ok_or("private fork file map")?;
    need(
        rows.len() == 95
            && provenance["fork_files"] == 95
            && provenance["upstream"]["pristine_files"] == 94
            && provenance["unchanged_pristine_files"] == 90,
        "finite fork count",
    )?;
    let vendor = root.join(VENDOR);
    let mut expected = BTreeSet::new();
    let mut pristine = 0;
    for row in rows {
        let name = row["path"].as_str().ok_or("private member path")?;
        need(
            relative(name) && expected.insert(name.to_owned()),
            "duplicate or escaped member",
        )?;
        let full = format!("{VENDOR}/{name}");
        let bytes = read(
            root,
            &full,
            Some(row["bytes"].as_u64().ok_or("private member size")?),
        )?;
        need(super::hash(&bytes) == row["sha256"], "member changed")?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let expected_mode = u32::from_str_radix(row["mode"].as_str().ok_or("member mode")?, 8)
                .map_err(|e| e.to_string())?;
            need(
                fs::symlink_metadata(root.join(&full))
                    .map_err(|e| e.to_string())?
                    .permissions()
                    .mode()
                    & 0o7777
                    == expected_mode,
                "member Unix mode changed",
            )?;
        }
        if row["pristine"] == true {
            pristine += 1;
        }
    }
    need(pristine == 90, "pristine count")?;
    let mut actual = BTreeSet::new();
    let mut pending = vec![vendor.clone()];
    let mut directory_count = 0;
    while let Some(directory) = pending.pop() {
        directory_count += 1;
        need(directory_count <= 128, "directory count exceeded")?;
        for entry in fs::read_dir(directory).map_err(|e| e.to_string())? {
            let path = entry.map_err(|e| e.to_string())?.path();
            let metadata = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
            need(!metadata.file_type().is_symlink(), "symlink member")?;
            if metadata.is_dir() {
                pending.push(path);
            } else {
                need(
                    metadata.is_file() && actual.len() < 96,
                    "non-file or extra member",
                )?;
                let name = path
                    .strip_prefix(&vendor)
                    .map_err(|e| e.to_string())?
                    .to_str()
                    .ok_or("non-UTF8 fork member")?
                    .replace('\\', "/");
                need(actual.insert(name), "duplicate physical member")?;
            }
        }
    }
    need(actual == expected, "missing or extra member")?;
    for (name, checksum) in [
        (
            "LICENSE-APACHE",
            "62c7a1e35f56406896d7aa7ca52d0cc0d272ac022b5d2796e7d6905db8a3636a",
        ),
        (
            "LICENSE-MIT",
            "23f18e03dc49df91622fe2a76176497404e46ced8a715d9d2b67a7446571cca3",
        ),
    ] {
        need(
            super::hash(&read(root, &format!("{VENDOR}/{name}"), None)?) == checksum
                && provenance["licenses"][name] == checksum,
            "original license changed",
        )?;
    }
    Ok(())
}

fn root_graph(root: &Path, metadata: &Value) -> Result<Value> {
    const IDENTITY_ERROR: &str =
        "exact canonical private package missing, duplicate or wrong source";
    const REGISTRY: &str = "registry+https://github.com/rust-lang/crates.io-index";
    let wanted = root
        .join(VENDOR)
        .join("Cargo.toml")
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let packages = metadata["packages"]
        .as_array()
        .ok_or("private root packages")?;
    let mut private = Vec::new();
    let mut registry = Vec::new();
    for package in packages {
        if package["name"] != "process-wrap" || package["version"] != "10.0.1" {
            continue;
        }
        let id = package["id"].as_str().ok_or("private package ID")?;
        if package["source"].is_null() {
            let manifest = package["manifest_path"]
                .as_str()
                .ok_or("private canonical manifest")?;
            if id.starts_with("path+file:")
                && Path::new(manifest)
                    .canonicalize()
                    .map_err(|e| e.to_string())?
                    == wanted
            {
                private.push(id);
            }
        } else if package["source"] == REGISTRY {
            registry.push(id);
        }
    }
    need(private.len() == 1 && registry.len() == 1, IDENTITY_ERROR)?;
    let private_id = private[0];
    let registry_id = registry[0];
    let mut private_parents = BTreeSet::new();
    let mut registry_parents = BTreeSet::new();
    let expected = ["crates/xtask/Cargo.toml", "crates/ssx-cli-check/Cargo.toml"]
        .into_iter()
        .map(|p| root.join(p).canonicalize().map_err(|e| e.to_string()))
        .collect::<Result<BTreeSet<_>>>()?;
    for node in metadata["resolve"]["nodes"]
        .as_array()
        .ok_or("private root resolve nodes")?
    {
        for dep in node["deps"]
            .as_array()
            .ok_or("private root dependency edges")?
        {
            let is_private = dep["pkg"] == private_id;
            let is_registry = dep["pkg"] == registry_id;
            if !is_private && !is_registry {
                continue;
            }
            let parent = packages
                .iter()
                .find(|p| p["id"] == node["id"])
                .ok_or("private edge parent")?;
            if is_private {
                need(
                    parent["source"].is_null(),
                    "private fork reached by registry package",
                )?;
                let manifest = parent["manifest_path"]
                    .as_str()
                    .ok_or("private consumer manifest")?;
                private_parents.insert(
                    Path::new(manifest)
                        .canonicalize()
                        .map_err(|e| e.to_string())?,
                );
            } else {
                need(
                    parent["name"] == "rmcp"
                        && parent["version"] == "3.5.1"
                        && parent["source"] == REGISTRY,
                    "upstream registry consumer changed",
                )?;
                registry_parents.insert(parent["id"].as_str().ok_or("registry consumer ID")?);
            }
        }
    }
    need(
        private_parents == expected && registry_parents.len() == 1,
        "exact two private and one registry consumer edges",
    )?;
    Ok(
        json!({"private_package_id":private_id,"private_manifest":wanted,"private_consumer_manifests":private_parents,"retained_registry_package_id":registry_id,"registry_rmcp_consumer":registry_parents,"scope":"private developer owners only; no production rmcp job-accounting fix"}),
    )
}

pub(super) fn qualify(
    root: &Path,
    host: &str,
    target: &Path,
    records: &mut Vec<Value>,
) -> Result<()> {
    let target = target.to_str().ok_or("private qualification target UTF8")?;
    let manifest = format!("{VENDOR}/Cargo.toml");
    // The excluded private package has its own pristine upstream lock. These
    // roles are not represented as the root consumers' dependency resolution.
    let metadata_raw = command(
        root,
        "cargo",
        &[
            "metadata",
            "--locked",
            "--format-version",
            "1",
            "--filter-platform",
            host,
        ],
        records,
    )?;
    let metadata: Value = serde_json::from_str(&metadata_raw).map_err(|e| e.to_string())?;
    let graph = root_graph(root, &metadata)?;
    // Validate the actual root graph FIRST, then alter only the private package
    // identity field. Neither name/version matching nor a registry copy suffices.
    let position = metadata["packages"]
        .as_array()
        .ok_or("private packages")?
        .iter()
        .position(|p| p["id"] == graph["private_package_id"])
        .ok_or("private actual package")?;
    let mut controls = Vec::new();
    for field in ["source", "manifest_path"] {
        let mut mutant = metadata.clone();
        let replacement = if field == "source" {
            json!("registry+https://github.com/rust-lang/crates.io-index")
        } else {
            json!(root.join("Cargo.toml"))
        };
        mutant["packages"][position][field] = replacement;
        let rejection = root_graph(root, &mutant).err();
        let expected = "T08A private process-wrap: exact canonical private package missing, duplicate or wrong source";
        need(
            rejection.as_deref() == Some(expected),
            "wrong private identity was not rejected for its exact reason",
        )?;
        controls.push(json!({"pointer":format!("/packages/{position}/{field}"),"mutant_sha256":super::hash(&serde_json::to_vec(&mutant).map_err(|e| e.to_string())?),"exact_rejection":expected}));
    }
    records.push(json!({"plan":"T08A","private_process_wrap_root_graph":{"actual_raw_metadata_sha256":super::hash(metadata_raw.as_bytes()),"actual_serialized_metadata_sha256":super::hash(&serde_json::to_vec(&metadata).map_err(|e| e.to_string())?),"positive_validated_before_mutants":true,"actual_identity":graph,"single_field_negative_controls":controls}}));
    let first = records.len();
    command(
        root,
        "cargo",
        &[
            "clippy",
            "--locked",
            "--manifest-path",
            &manifest,
            "--target-dir",
            target,
            "--target",
            host,
            "--no-default-features",
            "--features",
            FEATURES,
            "--lib",
            "--",
            "-D",
            "warnings",
        ],
        records,
    )?;
    if cfg!(windows) {
        let mut args = vec![
            "test",
            "--locked",
            "--manifest-path",
            &manifest,
            "--target-dir",
            target,
            "--target",
            host,
            "--all-features",
            "--test",
            "tokio_windows",
            "--",
            "--list",
            "--format",
            "terse",
        ];
        let listing = command(root, "cargo", &args, records)?;
        let inventory: Value = serde_json::from_str(include_str!(
            "../../../docs/rust-rewrite/t08a/process-wrap-windows-controls.json"
        ))
        .map_err(|e| e.to_string())?;
        let expected = inventory["tests"]
            .as_array()
            .ok_or("private Windows inventory")?
            .iter()
            .map(|n| n.as_str().ok_or("private test name"))
            .collect::<std::result::Result<BTreeSet<_>, _>>()?;
        let actual = listing
            .lines()
            .filter_map(|l| l.strip_suffix(": test"))
            .collect::<Vec<_>>();
        need(
            actual.len() == 30
                && expected.len() == 30
                && actual.iter().copied().collect::<BTreeSet<_>>() == expected,
            "Windows test inventory missing, duplicate or changed",
        )?;
        args.truncate(args.len() - 3);
        args.extend(["--test-threads=1"]);
        let unit = command(root, "cargo", &args, records)?;
        let ignored = inventory["original_ignored"]
            .as_array()
            .ok_or("private original ignored tests")?
            .iter()
            .map(|n| n.as_str().ok_or("private ignored name"))
            .collect::<std::result::Result<BTreeSet<_>, _>>()?;
        need(ignored.len() == 2, "original ignored count")?;
        for name in expected {
            let marker = if ignored.contains(name) {
                format!("test {name} ... ignored")
            } else {
                format!("test {name} ... ok")
            };
            need(
                unit.lines()
                    .filter(|line| {
                        if ignored.contains(name) {
                            *line == marker || line.starts_with(&(marker.clone() + ","))
                        } else {
                            *line == marker
                        }
                    })
                    .count()
                    == 1,
                "Windows test did not execute exactly once",
            )?;
        }
        need(
            unit.contains("28 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out"),
            "unfiltered Windows regression summary",
        )?;
    }
    records.push(json!({"plan":"T08A", "private_process_wrap_qualification":{
        "archive_sha256":CHECKSUM,"fork_files":95,"first_actual_record":first,
        "native_target":host,"shared_target_directory":target,
        "private_upstream_lock_sha256":super::hash(&read(root,&format!("{VENDOR}/Cargo.lock"),None)?),
        "actual_consumer_root_lock_sha256":super::hash(&read(root,"Cargo.lock",None)?),
        "private_windows_tests_executed":cfg!(windows),
        "scope":"excluded private LIB strict lint; Windows exact30/unfiltered28PASS+2originalignored; actual root consumers separately execute T08A whole-job cleanup"}}));
    Ok(())
}
