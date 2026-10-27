//! Genuine vendored Reqwest and SDK unit roots, sharing bounded source preparation.
use crate::Result;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};
const DECLARATION: &str = "docs/rust-rewrite/t07/unit-profile/current-declaration.json";
const DECLARATION_SHA: &str = "e07d55b51f99479038041fa8a49d862c0aaa1030e3296756308b8317d270b12f";
const SDK_DECLARATION: &str = "docs/rust-rewrite/t07/unit-profile/sdk-current-declaration.json";
const SDK_DECLARATION_SHA: &str =
    "3562c6f60afb5e68f24c5fb225fc11db436402c1e50ad7046a75956bc6adabba";
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Reqwest,
    Sdk,
}
impl Kind {
    fn declaration(self) -> &'static str {
        match self {
            Self::Reqwest => DECLARATION,
            Self::Sdk => SDK_DECLARATION,
        }
    }
    fn digest(self) -> &'static str {
        match self {
            Self::Reqwest => DECLARATION_SHA,
            Self::Sdk => SDK_DECLARATION_SHA,
        }
    }
}
fn sdk(doc: &Value) -> bool {
    doc["kind"] == "T07_GENUINE_SDK_UNIT_PROFILE_V1"
}
fn features(doc: &Value) -> Result<String> {
    rows(doc, "requested_features")?
        .iter()
        .map(|v| {
            v.as_str()
                .ok_or_else(|| "unit requested feature text".into())
        })
        .collect::<Result<Vec<_>>>()
        .map(|v| v.join(","))
}
fn digest(raw: &[u8]) -> String {
    Sha256::digest(raw)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn need(ok: bool, message: &str) -> Result<()> {
    if ok { Ok(()) } else { Err(message.into()) }
}
fn text<'a>(row: &'a Value, key: &str) -> Result<&'a str> {
    row[key]
        .as_str()
        .ok_or_else(|| format!("unit profile {key} absent"))
}
fn rows<'a>(value: &'a Value, key: &str) -> Result<&'a Vec<Value>> {
    value[key]
        .as_array()
        .ok_or_else(|| format!("unit profile {key} absent"))
}
fn relative(name: &str) -> Result<&Path> {
    let path = Path::new(name);
    need(
        !name.is_empty()
            && !name.contains('\\')
            && !name.contains(':')
            && !name
                .split('/')
                .any(|p| p.is_empty() || p == "." || p == "..")
            && path.components().all(|c| matches!(c, Component::Normal(_))),
        "unit profile unsafe member",
    )?;
    Ok(path)
}
fn checked(root: &Path, row: &Value) -> Result<Vec<u8>> {
    let path = root.join(relative(text(row, "path")?)?);
    let meta = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
    need(
        meta.is_file() && !meta.file_type().is_symlink(),
        "unit profile member type",
    )?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        need(
            format!("{:#o}", meta.permissions().mode() & 0o777) == text(row, "mode")?,
            "unit profile source mode",
        )?;
    }
    let raw = fs::read(path).map_err(|e| e.to_string())?;
    need(
        row["bytes"].as_u64() == Some(raw.len() as u64) && text(row, "sha256")? == digest(&raw),
        "unit profile source identity",
    )?;
    Ok(raw)
}
fn members(root: &Path, directory: &Path, output: &mut BTreeSet<String>) -> Result<()> {
    for entry in fs::read_dir(directory).map_err(|e| e.to_string())? {
        let path = entry.map_err(|e| e.to_string())?.path();
        let meta = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
        need(!meta.file_type().is_symlink(), "unit profile source link")?;
        if meta.is_dir() {
            members(root, &path, output)?;
        } else {
            need(meta.is_file(), "unit profile special source")?;
            output.insert(
                path.strip_prefix(root)
                    .map_err(|e| e.to_string())?
                    .to_str()
                    .ok_or("unit profile UTF8 path")?
                    .replace('\\', "/"),
            );
        }
    }
    Ok(())
}
#[cfg(test)]
fn document(raw: &[u8]) -> Result<Value> {
    document_for(raw, Kind::Reqwest)
}
fn document_for(raw: &[u8], kind: Kind) -> Result<Value> {
    need(
        digest(raw) == kind.digest(),
        "unit profile closed declaration digest",
    )?;
    let doc: Value = serde_json::from_slice(raw).map_err(|e| e.to_string())?;
    need(
        doc["schema_version"] == 1
            && doc["default_features"] == true
            && match kind {
                Kind::Reqwest => {
                    doc["kind"] == "T07_GENUINE_REQWEST_UNIT_PROFILE_V1"
                        && doc["requested_features"] == json!(["ssx-http1-source-pool"])
                        && rows(&doc, "metadata_dependency_declarations")?.len() == 67
                        && rows(&doc, "targets")?.len() == 1
                }
                Kind::Sdk => {
                    sdk(&doc)
                        && rows(&doc, "package_files")?.len() == 174
                        && rows(&doc, "metadata_dependency_declarations")?.len() == 49
                        && rows(&doc, "metadata_dependency_declarations")?
                            .iter()
                            .filter(|v| v["kind"] == "dev")
                            .count()
                            == 12
                        && rows(&doc, "targets")?.len() == 85
                        && rows(&doc, "native_test_inventory")?.len() == 580
                        && rows(&doc, "requested_features")?
                            .iter()
                            .any(|v| v == "ssx-source-metadata")
                }
            },
        "unit profile closed request",
    )?;
    Ok(doc)
}
fn source(root: &Path, doc: &Value) -> Result<()> {
    checked(root, &doc["historical_declaration"])?;
    for input in rows(doc, "workspace_source_roots")? {
        let mut actual = BTreeSet::new();
        members(
            root,
            &root.join(relative(text(input, "root")?)?),
            &mut actual,
        )?;
        let mut expected = BTreeSet::new();
        for row in rows(input, "files")? {
            need(
                expected.insert(text(row, "path")?.to_owned()),
                "unit duplicate workspace input",
            )?;
            checked(root, row)?;
        }
        need(
            actual == expected,
            "unit complete original-root workspace input closure",
        )?;
    }
    need(
        doc["original_manifest_declarations"]["package"]["name"]
            == if sdk(doc) { "rmcp" } else { "reqwest" }
            && doc["original_manifest_declarations"]["package"]["version"]
                == if sdk(doc) { "3.5.1" } else { "0.13.5" },
        "unit profile package identity",
    )?;
    for fork in rows(doc, "forks")? {
        let directory = root.join(relative(text(fork, "root")?)?);
        let meta = fs::symlink_metadata(&directory).map_err(|e| e.to_string())?;
        need(
            meta.is_dir() && !meta.file_type().is_symlink(),
            "unit source fork directory type",
        )?;
        let mut actual = BTreeSet::new();
        members(root, &directory, &mut actual)?;
        let mut expected = BTreeSet::new();
        let mut folded = BTreeSet::new();
        for row in rows(fork, "files")? {
            need(
                folded.insert(text(row, "path")?.to_lowercase()),
                "unit profile case-alias source row",
            )?;
            need(
                expected.insert(text(row, "path")?.to_owned()),
                "unit profile duplicate source row",
            )?;
            checked(root, row)?;
        }
        need(actual == expected, "unit profile complete fork membership")?;
        need(
            digest(&fs::read(directory.join("Cargo.toml")).map_err(|e| e.to_string())?)
                == text(fork, "manifest_sha256")?,
            "unit profile fork manifest",
        )?;
    }
    Ok(())
}
fn toml_path(path: &Path) -> Result<String> {
    serde_json::to_string(path.to_str().ok_or("unit profile UTF8 canonical path")?)
        .map_err(|e| e.to_string())
}
fn manifest(root: &Path, package: &Path, doc: &Value) -> Result<Vec<u8>> {
    let mut raw = fs::read(root.join(text(doc, "package_root")?).join("Cargo.toml"))
        .map_err(|e| e.to_string())?;
    // The only path-bearing declaration added by the current source. Preserve
    // the entire genuine manifest, including every original dev/default feature.
    if !sdk(doc) {
        let raw_text = std::str::from_utf8(&raw).map_err(|e| e.to_string())?;
        let from = "path = \"../../../crates/ssx-source-codecs\"";
        need(
            raw_text.matches(from).count() == 1,
            "unit exact sole codec path rewrite",
        )?;
        let original = root
            .join("crates/ssx-source-codecs")
            .canonicalize()
            .map_err(|e| e.to_string())?;
        raw = raw_text
            .replacen(from, &format!("path = {}", toml_path(&original)?), 1)
            .into_bytes();
    }
    let mut suffix = "\n[workspace]\nresolver = \"3\"\n\n[patch.crates-io]\n".to_owned();
    for fork in rows(doc, "forks")? {
        let name = text(fork, "name")?;
        let path =
            if Some(name) == doc["original_manifest_declarations"]["package"]["name"].as_str() {
                package.to_owned()
            } else {
                root.join(relative(text(fork, "root")?)?)
                    .canonicalize()
                    .map_err(|e| e.to_string())?
            };
        suffix.push_str(&format!("{name} = {{ path = {} }}\n", toml_path(&path)?));
    }
    raw.extend_from_slice(suffix.as_bytes());
    Ok(raw)
}
fn lock(root: &Path, doc: &Value) -> Result<Vec<u8>> {
    let raw =
        fs::read(root.join(relative(text(&doc["lock"], "path")?)?)).map_err(|e| e.to_string())?;
    need(
        doc["lock"]["bytes"].as_u64() == Some(raw.len() as u64)
            && digest(&raw) == text(&doc["lock"], "sha256")?,
        "unit profile retained Cargo lock",
    )?;
    Ok(raw)
}
fn destination(root: &Path, path: &Path) -> Result<PathBuf> {
    need(
        path.is_absolute() && !path.components().any(|p| p.as_os_str() == ".git"),
        "unit profile must be an absolute owned non-metadata path",
    )?;
    let parent = path
        .parent()
        .ok_or("unit profile parent")?
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or("unit profile destination name")?;
    relative(name)?;
    let source = root.canonicalize().map_err(|e| e.to_string())?;
    let dest = parent.join(name);
    need(
        !dest.starts_with(&source) && !source.starts_with(&dest),
        "unit profile must be outside checkout",
    )?;
    Ok(dest)
}
pub struct Profile {
    directory: PathBuf,
    package: PathBuf,
    root: PathBuf,
    doc: Value,
    kind: Kind,
    lease: Vec<u8>,
    retain_lease: std::cell::Cell<bool>,
}
impl Drop for Profile {
    fn drop(&mut self) {
        let path = self.directory.join("active-owner.json");
        if !self.retain_lease.get()
            && fs::read(&path).ok().as_deref() == Some(self.lease.as_slice())
        {
            let _ = fs::remove_file(path);
        }
    }
}
impl Profile {
    fn verify(&self) -> Result<()> {
        source(&self.root, &self.doc)?;
        let meta = fs::symlink_metadata(&self.package).map_err(|e| e.to_string())?;
        need(
            meta.is_dir() && !meta.file_type().is_symlink(),
            "unit copied package directory type",
        )?;
        let mut expected = BTreeSet::new();
        for row in rows(&self.doc, "package_files")? {
            let name = text(row, "path")?;
            expected.insert(name.to_owned());
            let raw = match name {
                "Cargo.toml" => manifest(&self.root, &self.package, &self.doc)?,
                "Cargo.lock" => lock(&self.root, &self.doc)?,
                _ => checked(&self.root.join(text(&self.doc, "package_root")?), row)?,
            };
            let path = self.package.join(relative(name)?);
            let meta = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
            need(
                meta.is_file()
                    && !meta.file_type().is_symlink()
                    && fs::read(&path).map_err(|e| e.to_string())? == raw,
                "unit profile copied source/manifest/lock identity",
            )?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                need(
                    format!("{:#o}", meta.permissions().mode() & 0o777) == text(row, "mode")?,
                    "unit profile copied executable mode",
                )?;
            }
        }
        let mut actual = BTreeSet::new();
        members(&self.package, &self.package, &mut actual)?;
        need(
            actual == expected,
            "unit profile copied complete membership",
        )?;
        need(
            fs::read(self.directory.join("declaration.json")).map_err(|e| e.to_string())?
                == fs::read(self.root.join(self.kind.declaration())).map_err(|e| e.to_string())?,
            "unit profile owned marker",
        )
    }
    pub fn open(root: &Path, path: &Path, existing: bool) -> Result<Self> {
        Self::open_kind(root, path, existing, Kind::Reqwest)
    }
    pub fn open_sdk(root: &Path, path: &Path, existing: bool) -> Result<Self> {
        Self::open_kind(root, path, existing, Kind::Sdk)
    }
    fn open_kind(root: &Path, path: &Path, existing: bool, kind: Kind) -> Result<Self> {
        let doc = document_for(
            &fs::read(root.join(kind.declaration())).map_err(|e| e.to_string())?,
            kind,
        )?;
        source(root, &doc)?;
        let directory = destination(root, path)?;
        if existing {
            let meta = fs::symlink_metadata(&directory).map_err(|e| e.to_string())?;
            need(
                meta.is_dir() && !meta.file_type().is_symlink(),
                "unit profile existing directory type",
            )?;
            let marker = directory.join("declaration.json");
            let meta = fs::symlink_metadata(&marker).map_err(|e| e.to_string())?;
            need(
                meta.is_file()
                    && !meta.file_type().is_symlink()
                    && fs::read(marker).map_err(|e| e.to_string())?
                        == fs::read(root.join(kind.declaration())).map_err(|e| e.to_string())?,
                "unit foreign profile marker",
            )?;
            let receipts =
                fs::symlink_metadata(directory.join("receipts")).map_err(|e| e.to_string())?;
            need(
                receipts.is_dir() && !receipts.file_type().is_symlink(),
                "unit foreign receipts directory",
            )?;
        } else {
            fs::create_dir(&directory)
                .map_err(|e| format!("unit profile exclusive destination: {e}"))?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))
                    .map_err(|e| e.to_string())?;
            }
        }
        let lease = serde_json::to_vec(&json!({"pid":std::process::id(),"declaration":kind.digest(),"started":std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|e|e.to_string())?.as_nanos()})).map_err(|e| e.to_string())?;
        let mut owner = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(directory.join("active-owner.json"))
            .map_err(|e| format!("unit profile active/foreign writer: {e}"))?;
        owner.write_all(&lease).map_err(|e| e.to_string())?;
        let profile = Self {
            package: directory.join("package"),
            directory,
            root: root.to_owned(),
            doc,
            kind,
            lease,
            retain_lease: std::cell::Cell::new(false),
        };
        if !existing {
            fs::create_dir(&profile.package).map_err(|e| e.to_string())?;
            fs::create_dir(profile.directory.join("receipts")).map_err(|e| e.to_string())?;
            for row in rows(&profile.doc, "package_files")? {
                let name = text(row, "path")?;
                let input = root
                    .join(text(&profile.doc, "package_root")?)
                    .join(relative(name)?);
                let dest = profile.package.join(relative(name)?);
                fs::create_dir_all(dest.parent().ok_or("unit profile file parent")?)
                    .map_err(|e| e.to_string())?;
                let raw = match name {
                    "Cargo.toml" => manifest(root, &profile.package, &profile.doc)?,
                    "Cargo.lock" => lock(root, &profile.doc)?,
                    _ => checked(&root.join(text(&profile.doc, "package_root")?), row)?,
                };
                let mut output = OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&dest)
                    .map_err(|e| e.to_string())?;
                output.write_all(&raw).map_err(|e| e.to_string())?;
                fs::set_permissions(
                    &dest,
                    fs::metadata(&input)
                        .map_err(|e| e.to_string())?
                        .permissions(),
                )
                .map_err(|e| e.to_string())?;
            }
            fs::write(
                profile.directory.join("declaration.json"),
                fs::read(root.join(kind.declaration())).map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;
        }
        profile.verify()?;
        Ok(profile)
    }
    fn next(&self) -> Result<PathBuf> {
        for n in 0..10000 {
            let path = self
                .directory
                .join("receipts")
                .join(format!("{n:05}.command.json"));
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(_) => return Ok(path),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(e) => return Err(e.to_string()),
            }
        }
        Err("unit profile command receipt limit".into())
    }
    fn command(&self, cwd: &Path, args: &[String], records: &mut Vec<Value>) -> Result<String> {
        need(
            cfg!(unix),
            "unit native process ownership qualification currently Unix; Windows preparation only",
        )?;
        let cargo = Path::new(env!("CARGO"));
        need(cargo.is_absolute(), "unit pinned Cargo path")?;
        let receipt = self.next()?;
        let stdout_path = receipt.with_extension("stdout");
        let stderr_path = receipt.with_extension("stderr");
        let stdout = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&stdout_path)
            .map_err(|e| e.to_string())?;
        let stderr = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&stderr_path)
            .map_err(|e| e.to_string())?;
        let mut command = Command::new(cargo);
        command
            .current_dir(cwd)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
        }
        let child = command.spawn().map_err(|e| e.to_string())?;
        let mut owned = OwnedChild::new(child, &self.retain_lease);
        let pid = owned.child.id();
        let start = Instant::now();
        let limits = &self.doc["command_limits"];
        let mut stop: Option<String> = None;
        let (send, receive) = std::sync::mpsc::channel();
        let pipe = owned.child.stdout.take().ok_or("unit child stdout")?;
        owned.readers.push(capture(
            pipe,
            stdout,
            limits["stdout_bytes"].as_u64().ok_or("unit stdout cap")?,
            send.clone(),
        ));
        let pipe = owned.child.stderr.take().ok_or("unit child stderr")?;
        owned.readers.push(capture(
            pipe,
            stderr,
            limits["stderr_bytes"].as_u64().ok_or("unit stderr cap")?,
            send,
        ));
        let status = loop {
            if let Some(status) = owned.child.try_wait().map_err(|e| e.to_string())? {
                break status;
            }
            if start.elapsed()
                >= Duration::from_secs(limits["seconds"].as_u64().ok_or("unit timeout")?)
            {
                stop = Some("unit command timeout".into());
            }
            if let Ok(error) = receive.try_recv() {
                stop = Some(error);
            }
            if stop.is_some() {
                if let Err(error) = owned.finish() {
                    stop = Some(format!(
                        "{}; {error}",
                        stop.as_deref().unwrap_or("unit stop")
                    ));
                }
                break owned.child.wait().map_err(|e| e.to_string())?;
            }
            std::thread::sleep(Duration::from_millis(50));
        };
        if let Err(error) = owned.finish() {
            stop.get_or_insert(error);
        }
        let waited = owned.child.wait().map_err(|e| e.to_string())?;
        let environment = unit_environment();
        let row = json!({"program":cargo,"args":args,"cwd":cwd,"environment":environment,"exit_code":status.code(),"wait_exit_code":waited.code(),"original_pid":pid,"waited":true,"original_group_absent":owned.group_absent,"group_probes":owned.probes,"readers_joined":owned.readers.is_empty(),"stop":stop,"stdout_path":stdout_path,"stderr_path":stderr_path,"elapsed_ms":start.elapsed().as_millis()});
        fs::write(
            &receipt,
            serde_json::to_vec_pretty(&row).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        records.push(row);
        need(
            status.success() && stop.is_none() && owned.group_absent,
            "unit Cargo child failed; see original-handle retirement/raw receipts",
        )?;
        fs::read_to_string(stdout_path).map_err(|e| e.to_string())
    }
    fn args(&self, role: &str, offline: bool) -> Vec<String> {
        let mut args = vec![
            "--config".into(),
            "resolver.incompatible-rust-versions=\"allow\"".into(),
            role.into(),
            "--locked".into(),
        ];
        if offline {
            args.push("--offline".into());
        }
        args
    }
    pub fn compile(&self, offline: bool, records: &mut Vec<Value>) -> Result<()> {
        self.verify()?;
        let mut metadata = self.args("metadata", offline);
        metadata.extend(["--no-deps".into(), "--format-version=1".into()]);
        let value: Value = serde_json::from_str(&self.command(&self.root, &metadata, records)?)
            .map_err(|e| e.to_string())?;
        let target = PathBuf::from(text(&value, "target_directory")?);
        need(target.is_absolute(), "unit root metadata target path")?;
        if self.kind == Kind::Reqwest {
            let mut args = self.args("test", offline);
            args.extend(
                [
                    "--no-run",
                    "-p",
                    "ssx-mcp",
                    "-p",
                    "ssx-gateway",
                    "-p",
                    "xtask",
                    "--all-targets",
                    "--features",
                    "ssx-gateway/source-endpoint-test-fixtures",
                    "--message-format=json",
                ]
                .map(str::to_owned),
            );
            args.push("--target-dir".into());
            args.push(target.to_str().ok_or("unit target UTF8")?.into());
            let output = self.command(&self.root, &args, records)?;
            no_python_units(&output)?;
            for line in output.lines().filter(|line| !line.trim().is_empty()) {
                let unit: Value = serde_json::from_str(line).map_err(|e| e.to_string())?;
                if unit["reason"] == "compiler-artifact" && unit["target"]["name"] == "serde_json" {
                    need(
                        !rows(&unit, "features")?
                            .iter()
                            .any(|f| f == "arbitrary_precision"),
                        "production serde arbitrary_precision forbidden",
                    )?;
                }
            }
        }
        let mut metadata = self.args("metadata", offline);
        metadata.extend([
            "--no-deps".into(),
            "--format-version=1".into(),
            "--manifest-path".into(),
            self.package
                .join("Cargo.toml")
                .to_str()
                .ok_or("unit manifest UTF8")?
                .into(),
            "--features".into(),
            features(&self.doc)?,
            "--config".into(),
            format!("build.target-dir={}", toml_path(&target)?),
        ]);
        let value: Value = serde_json::from_str(&self.command(&self.root, &metadata, records)?)
            .map_err(|e| e.to_string())?;
        let packages = rows(&value, "packages")?;
        need(packages.len() == 1, "unit genuine root inventory")?;
        let package = &packages[0];
        let mut dependencies = self.doc["metadata_dependency_declarations"].clone();
        if self.kind == Kind::Reqwest {
            let codec = dependencies
                .as_array_mut()
                .ok_or("unit dependency declarations")?
                .iter_mut()
                .find(|d| d["name"] == "ssx-source-codecs")
                .ok_or("unit codec declaration")?;
            need(
                codec["path"] == "@SOURCE_ROOT@/crates/ssx-source-codecs",
                "unit codec path marker",
            )?;
            codec["path"] = json!(
                self.root
                    .join("crates/ssx-source-codecs")
                    .canonicalize()
                    .map_err(|e| e.to_string())?
            );
        }
        let mut expected = self.doc["targets"].clone();
        for target in expected.as_array_mut().ok_or("unit target declarations")? {
            let relative = text(target, "src_path")?
                .strip_prefix("@PACKAGE_ROOT@/")
                .map(str::to_owned);
            if let Some(relative) = relative {
                target["src_path"] = json!(self.package.join(relative));
            }
        }
        if self.kind == Kind::Reqwest {
            expected[0]["src_path"] = json!(self.package.join("src/lib.rs"));
        }
        need(
            package["name"] == self.doc["original_manifest_declarations"]["package"]["name"]
                && package["version"]
                    == self.doc["original_manifest_declarations"]["package"]["version"]
                && package["dependencies"] == dependencies
                && package["features"] == self.doc["metadata_feature_declarations"]
                && package["targets"] == expected,
            "unit genuine declarations/features/complete targets",
        )?;
        let mut args = self.args("test", offline);
        args.extend([
            "--manifest-path".into(),
            self.package
                .join("Cargo.toml")
                .to_str()
                .ok_or("unit manifest UTF8")?
                .into(),
            "--features".into(),
            features(&self.doc)?,
            if self.kind == Kind::Sdk {
                "--lib".into()
            } else {
                "--all-targets".into()
            },
            "--no-run".into(),
            "--message-format=json".into(),
            "--target-dir".into(),
            target.to_str().ok_or("unit target UTF8")?.into(),
        ]);
        let output = self.command(&self.root, &args, records)?;
        no_python_units(&output)?;
        let executable = artifact(
            &output,
            text(package, "id")?,
            &self.package.join("src/lib.rs"),
            &target,
            &selected_features(&self.doc)?,
        )?;
        let marker = json!({"target":target,"executable":executable,"binary_sha256":file_digest(&executable)?,"package_id":package["id"],"environment":unit_environment(),"declaration_sha256":self.kind.digest()});
        publish(
            &self.directory,
            "compiled-unit.json",
            &serde_json::to_vec(&marker).map_err(|e| e.to_string())?,
        )?;
        self.verify()
    }
    pub fn tests(&self, expected: &[Value], records: &mut Vec<Value>) -> Result<()> {
        need(
            expected.len() == if self.kind == Kind::Sdk { 32 } else { 19 },
            "unit exact finite controls required",
        )?;
        self.verify()?;
        let target: Value = serde_json::from_slice(
            &fs::read(self.directory.join("compiled-unit.json")).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        need(
            target["environment"] == unit_environment()
                && target["declaration_sha256"] == self.kind.digest(),
            "unit compiled environment/declaration changed",
        )?;
        let executable = Path::new(text(&target, "executable")?);
        need(
            file_digest(executable)? == text(&target, "binary_sha256")?,
            "unit compiled image changed",
        )?;
        if self.kind == Kind::Sdk {
            let names = rows(&self.doc, "native_test_inventory")?
                .iter()
                .map(|v| {
                    v.as_str()
                        .map(str::to_owned)
                        .ok_or_else(|| "SDK fullinventory name".into())
                })
                .collect::<Result<BTreeSet<_>>>()?;
            need(names.len() == 580, "SDK full580 unique IDs")?;
            let mut args = self.args("test", true);
            args.extend([
                "--manifest-path".into(),
                self.package
                    .join("Cargo.toml")
                    .to_str()
                    .ok_or("SDK manifestUTF8")?
                    .into(),
                "--lib".into(),
                "--features".into(),
                features(&self.doc)?,
                "--target-dir".into(),
                text(&target, "target")?.into(),
                "--".into(),
                "--list".into(),
            ]);
            let output = self.command(&self.root, &args, records)?;
            inventory(&output, &names, true)?;
            need(
                output
                    .lines()
                    .filter(|s| *s == "580 tests, 0 benchmarks")
                    .count()
                    == 1,
                "SDK fullinventory exactsummary",
            )?;
        }
        let groups: &[(&str, usize)] = if self.kind == Kind::Sdk {
            &[
                ("transport::auth::source_metadata::", 14),
                ("transport::auth::source_manager::tests::", 7),
                (
                    "transport::auth::source_manager::source_operation_tests::",
                    11,
                ),
            ]
        } else {
            &[
                ("source_pool::source_cookie", 18),
                (
                    "source_pool::tests::caller_proxy_authorization_precedes_route_defaults_and_generated_values_never_become_callers",
                    1,
                ),
            ]
        };
        for &(filter, count) in groups {
            let names: BTreeSet<String> = expected
                .iter()
                .filter_map(Value::as_str)
                .filter(|s| s.starts_with(filter))
                .map(str::to_owned)
                .collect();
            need(names.len() == count, "unit declared selector group")?;
            let mut args = self.args("test", true);
            args.extend([
                "--manifest-path".into(),
                self.package
                    .join("Cargo.toml")
                    .to_str()
                    .ok_or("unit manifest UTF8")?
                    .into(),
                "--lib".into(),
                "--features".into(),
                features(&self.doc)?,
                "--target-dir".into(),
                text(&target, "target")?.into(),
                filter.into(),
                "--".into(),
            ]);
            if count == 1 {
                args.push("--exact".into());
            }
            let mut listing = args.clone();
            listing.push("--list".into());
            inventory(&self.command(&self.root, &listing, records)?, &names, true)?;
            args.push("--test-threads=1".into());
            let output = self.command(&self.root, &args, records)?;
            inventory(&output, &names, false)?;
            if self.kind == Kind::Sdk {
                need(
                    output.contains(&format!("; {} filtered out;", 580 - count)),
                    "SDK actual fullinventory filtered count",
                )?;
            }
        }
        need(
            file_digest(executable)? == text(&target, "binary_sha256")?,
            "unit compiled image changed after tests",
        )?;
        self.verify()
    }
}
fn publish(directory: &Path, name: &str, raw: &[u8]) -> Result<()> {
    relative(name)?;
    let temporary = directory.join(format!(".{name}.{}.new", std::process::id()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|e| e.to_string())?;
    file.write_all(raw).map_err(|e| e.to_string())?;
    drop(file);
    fs::rename(temporary, directory.join(name)).map_err(|e| e.to_string())
}
fn file_digest(path: &Path) -> Result<String> {
    let meta = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    need(
        meta.is_file() && !meta.file_type().is_symlink(),
        "unit binary type",
    )?;
    let mut file = File::open(path).map_err(|e| e.to_string())?;
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let n = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n]);
    }
    Ok(hash.finalize().iter().map(|b| format!("{b:02x}")).collect())
}
fn selected_features(doc: &Value) -> Result<BTreeSet<String>> {
    let definitions = doc["metadata_feature_declarations"]
        .as_object()
        .ok_or("unit feature definitions")?;
    let mut pending = vec!["default".to_owned()];
    for feature in rows(doc, "requested_features")? {
        pending.push(feature.as_str().ok_or("unit requestedfeature")?.to_owned());
    }
    let mut selected = BTreeSet::new();
    while let Some(feature) = pending.pop() {
        if let Some(values) = definitions.get(&feature)
            && selected.insert(feature)
        {
            for value in values.as_array().ok_or("unit feature expression list")? {
                let value = value.as_str().ok_or("unit feature expression")?;
                if definitions.contains_key(value) {
                    pending.push(value.to_owned());
                }
            }
        }
    }
    Ok(selected)
}
fn no_python_units(output: &str) -> Result<()> {
    for line in output.lines().filter(|line| !line.trim().is_empty()) {
        let value: Value = serde_json::from_str(line).map_err(|e| e.to_string())?;
        need(
            !(value["reason"] == "compiler-artifact"
                && value["package_id"]
                    .as_str()
                    .is_some_and(|p| p.contains("pyo3"))),
            "Python compiler unit forbidden",
        )?;
    }
    Ok(())
}
fn artifact(
    output: &str,
    package_id: &str,
    source: &Path,
    target: &Path,
    features: &BTreeSet<String>,
) -> Result<PathBuf> {
    let values: Vec<Value> = output
        .lines()
        .filter(|s| !s.trim().is_empty())
        .map(serde_json::from_str)
        .collect::<std::result::Result<_, _>>()
        .map_err(|e| e.to_string())?;
    let finished: Vec<_> = values
        .iter()
        .filter(|v| v["reason"] == "build-finished")
        .collect();
    need(
        finished.len() == 1 && finished[0]["success"] == true,
        "unit build not uniquely successful",
    )?;
    let matches: Vec<_> = values
        .iter()
        .filter(|v| {
            v["reason"] == "compiler-artifact"
                && v["package_id"] == package_id
                && v["target"]["src_path"].as_str() == source.to_str()
                && v["target"]["kind"] == json!(["lib"])
                && v["profile"]["test"] == true
                && v["executable"].is_string()
        })
        .collect();
    need(
        matches.len() == 1,
        "unit genuine test artifact absent/ambiguous",
    )?;
    let actual_features = rows(matches[0], "features")?
        .iter()
        .map(|v| {
            v.as_str()
                .map(str::to_owned)
                .ok_or("unit compiler feature encoding")
        })
        .collect::<std::result::Result<BTreeSet<_>, _>>()?;
    need(
        actual_features == *features && rows(matches[0], "features")?.len() == features.len(),
        "unit compiler requested/default feature closure",
    )?;
    let path = PathBuf::from(text(matches[0], "executable")?);
    let meta = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
    need(
        path.is_absolute() && meta.is_file() && !meta.file_type().is_symlink(),
        "unit test executable type",
    )?;
    need(
        path.canonicalize()
            .map_err(|e| e.to_string())?
            .starts_with(target.canonicalize().map_err(|e| e.to_string())?),
        "unit executable outside root Cargo target",
    )?;
    Ok(path)
}
fn unit_environment() -> Value {
    let mut values = serde_json::Map::new();
    for key in [
        "RUSTC",
        "RUSTDOC",
        "RUSTFLAGS",
        "CARGO_ENCODED_RUSTFLAGS",
        "CARGO_BUILD_TARGET",
        "CARGO_PROFILE_DEV_DEBUG",
        "CARGO_PROFILE_TEST_DEBUG",
        "CARGO_INCREMENTAL",
        "CARGO_HOME",
        "CARGO_TARGET_DIR",
    ] {
        values.insert(
            key.into(),
            std::env::var_os(key)
                .map(|v| Value::String(v.to_string_lossy().into_owned()))
                .unwrap_or(Value::Null),
        );
    }
    Value::Object(values)
}
fn capture(
    mut input: impl Read + Send + 'static,
    mut output: File,
    limit: u64,
    send: std::sync::mpsc::Sender<String>,
) -> std::thread::JoinHandle<Result<()>> {
    std::thread::spawn(move || {
        let result: Result<()> = (|| {
            let mut count = 0u64;
            let mut buf = [0u8; 65536];
            loop {
                let n = input.read(&mut buf).map_err(|e| e.to_string())?;
                if n == 0 {
                    return Ok(());
                }
                need(count + n as u64 <= limit, "unit output cap before write")?;
                output.write_all(&buf[..n]).map_err(|e| e.to_string())?;
                count += n as u64;
            }
        })();
        if let Err(error) = &result {
            let _ = send.send(error.clone());
        }
        result
    })
}
// A nonzero kill exit alone never proves ESRCH. These are the complete C-locale
// /bin/kill diagnostics for the supported macOS BSD and Linux procps variants.
fn probe_result(pid: u32, code: Option<i32>, stdout: &[u8], stderr: &[u8]) -> Result<bool> {
    need(stdout.is_empty(), "unit group probe unexpected stdout")?;
    if code == Some(0) && stderr.is_empty() {
        return Ok(false);
    }
    let absent = [
        format!("kill: -{pid}: No such process\n"),
        format!("kill: (-{pid}): No such process\n"),
        format!("/bin/kill: (-{pid}): No such process\n"),
    ];
    need(
        code == Some(1) && absent.iter().any(|s| s.as_bytes() == stderr),
        "unit group probe unknown status/diagnostic; ownership retained",
    )?;
    Ok(true)
}
struct OwnedChild<'a> {
    child: Child,
    readers: Vec<std::thread::JoinHandle<Result<()>>>,
    retired: bool,
    group_absent: bool,
    probes: Vec<Value>,
    finish_result: Option<Result<()>>,
    retain_lease: &'a std::cell::Cell<bool>,
    previous_retention: bool,
}
impl<'a> OwnedChild<'a> {
    fn new(child: Child, retain_lease: &'a std::cell::Cell<bool>) -> Self {
        // Latch before any pipe extraction, receipt write, or wait can fail.
        let previous_retention = retain_lease.replace(true);
        Self {
            child,
            readers: Vec::new(),
            retired: false,
            group_absent: false,
            probes: Vec::new(),
            finish_result: None,
            retain_lease,
            previous_retention,
        }
    }
    fn probe(&mut self) -> Result<bool> {
        let output = Command::new("/bin/kill")
            .env("LC_ALL", "C")
            .env("LANG", "C")
            .args(["-0", "--", &format!("-{}", self.child.id())])
            .output()
            .map_err(|e| format!("unit group probe launch: {e}"))?;
        self.probes.push(json!({"code":output.status.code(),"stdout":String::from_utf8_lossy(&output.stdout),"stderr":String::from_utf8_lossy(&output.stderr)}));
        probe_result(
            self.child.id(),
            output.status.code(),
            &output.stdout,
            &output.stderr,
        )
    }
    fn finish(&mut self) -> Result<()> {
        if let Some(result) = &self.finish_result {
            return result.clone();
        }
        let result = self.retire();
        // Preserve the first error, including reader/cap failures, on repeated calls.
        self.finish_result = Some(result.clone());
        result
    }
    fn retire(&mut self) -> Result<()> {
        #[cfg(unix)]
        {
            // The following exact probes, not this signal command's exit code,
            // establish absence. Child::kill also targets the original handle.
            let _ = Command::new("/bin/kill")
                .env("LC_ALL", "C")
                .env("LANG", "C")
                .args(["-KILL", "--", &format!("-{}", self.child.id())])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
        let _ = self.child.kill();
        self.child.wait().map_err(|e| e.to_string())?;
        self.retired = true;
        // Match the command monitor's existing polling cadence. A single absent
        // observation is insufficient; errors or a present group retain the lease.
        need(self.probe()?, "unit original group still present")?;
        std::thread::sleep(Duration::from_millis(50));
        need(self.probe()?, "unit original group absence not stable")?;
        self.group_absent = true;
        let mut errors = Vec::new();
        for reader in self.readers.drain(..) {
            match reader.join() {
                Ok(Ok(())) => {}
                Ok(Err(error)) => errors.push(error),
                Err(_) => errors.push("unit output reader panicked".into()),
            }
        }
        // A cap/reader error fails the command but all original resources retired.
        self.retain_lease.set(self.previous_retention);
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors.join("; "))
        }
    }
}
impl Drop for OwnedChild<'_> {
    fn drop(&mut self) {
        let _ = self.finish();
    }
}
fn summary(rest: &str, count: usize) -> Result<()> {
    let fields: Vec<_> = rest.split("; ").collect();
    need(
        fields.len() == 6
            && fields[0] == format!("result: ok. {count} passed")
            && fields[1] == "0 failed"
            && fields[2] == "0 ignored"
            && fields[3] == "0 measured",
        "unit exact successful summary",
    )?;
    fields[4]
        .strip_suffix(" filtered out")
        .ok_or("unit filtered summary")?
        .parse::<usize>()
        .map_err(|e| e.to_string())?;
    let elapsed = fields[5]
        .strip_prefix("finished in ")
        .and_then(|s| s.strip_suffix('s'))
        .ok_or("unit elapsed summary")?
        .parse::<f64>()
        .map_err(|e| e.to_string())?;
    need(
        elapsed.is_finite() && elapsed >= 0.0,
        "unit invalid elapsed summary",
    )
}
fn inventory(output: &str, expected: &BTreeSet<String>, listing: bool) -> Result<()> {
    let mut actual = BTreeMap::new();
    let mut summaries = 0;
    for line in output.lines() {
        if listing {
            if let Some(name) = line.strip_suffix(": test") {
                *actual.entry(name.to_owned()).or_insert(0usize) += 1;
            }
        } else if let Some(rest) = line.strip_prefix("test ") {
            if rest.starts_with("result: ") {
                summaries += 1;
                summary(rest, expected.len())?;
            } else {
                let (name, status) = rest.split_once(" ... ").ok_or("unit malformed result")?;
                need(status == "ok", "unit ignored/failed/unknown result")?;
                *actual.entry(name.to_owned()).or_insert(0usize) += 1;
            }
        }
    }
    need(
        actual.keys().cloned().collect::<BTreeSet<_>>() == *expected
            && actual.values().all(|n| *n == 1)
            && (listing || summaries == 1),
        "unit missing/duplicate/extra native IDs",
    )
}
pub fn cli(root: &Path, args: &[String]) -> bool {
    let Some(role) = args.first() else {
        return false;
    };
    if !matches!(
        role.as_str(),
        "prepare-t07-unit"
            | "bootstrap-t07-unit"
            | "prepare-t07-sdk-unit"
            | "bootstrap-t07-sdk-unit"
    ) {
        return false;
    }
    let result: Result<()> = (|| {
        need(
            args.len() == 3 && args[1] == "--profile-dir",
            "usage: cargo xtask prepare-t07-unit|bootstrap-t07-unit --profile-dir PATH",
        )?;
        let profile = if role.ends_with("sdk-unit") {
            Profile::open_sdk(root, Path::new(&args[2]), role.starts_with("bootstrap-"))?
        } else {
            Profile::open(root, Path::new(&args[2]), role == "bootstrap-t07-unit")?
        };
        let mut records = Vec::new();
        if role.starts_with("bootstrap-") {
            profile.compile(false, &mut records)?;
        }
        println!(
            "{}",
            json!({"profile":profile.directory,"package":profile.package,"commands":records,"python_executed":false})
        );
        Ok(())
    })();
    if let Err(error) = result {
        eprintln!("{error}");
        std::process::exit(1);
    }
    true
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn declaration_rejects_rehashed_changed_inputs() {
        let raw =
            include_bytes!("../../../docs/rust-rewrite/t07/unit-profile/current-declaration.json");
        assert!(document(raw).is_ok());
        let mut changed = raw.to_vec();
        changed.push(0);
        assert!(document(&changed).is_err());
        let sdk = include_bytes!(
            "../../../docs/rust-rewrite/t07/unit-profile/sdk-current-declaration.json"
        );
        assert!(document_for(sdk, Kind::Sdk).is_ok());
        assert!(document_for(sdk, Kind::Reqwest).is_err());
        let mut changed = sdk.to_vec();
        changed.push(0);
        assert!(document_for(&changed, Kind::Sdk).is_err());
    }
    #[test]
    fn path_controls_reject_cross_platform_escapes() {
        for p in ["../x", "/x", "a/../b", "a//b", "C:/x", "a\\b", ""] {
            assert!(relative(p).is_err(), "{p}");
        }
        assert!(relative("src/a.rs").is_ok());
    }
    #[test]
    fn group_probe_accepts_only_closed_pid_specific_esrch_and_clean_success() {
        assert!(!probe_result(42, Some(0), b"", b"").unwrap());
        for diagnostic in [
            "kill: -42: No such process\n",
            "kill: (-42): No such process\n",
            "/bin/kill: (-42): No such process\n",
        ] {
            assert!(probe_result(42, Some(1), b"", diagnostic.as_bytes()).unwrap());
            assert!(probe_result(42, Some(0), b"", diagnostic.as_bytes()).is_err());
            assert!(probe_result(42, Some(2), b"", diagnostic.as_bytes()).is_err());
            assert!(probe_result(43, Some(1), b"", diagnostic.as_bytes()).is_err());
        }
        for diagnostic in [
            "kill: -42: Operation not permitted\n",
            "kill: usage error\n",
            "unknown\n",
            "",
        ] {
            assert!(probe_result(42, Some(1), b"", diagnostic.as_bytes()).is_err());
        }
        assert!(probe_result(42, None, b"", b"").is_err());
        assert!(probe_result(42, Some(0), b"unexpected", b"").is_err());
        assert!(probe_result(42, Some(0), b"", b"warning").is_err());
    }
    #[test]
    fn native_inventory_rejects_ignored_duplicate_extra_and_missing_results() {
        let names = BTreeSet::from(["a".to_owned()]);
        let good = "test a ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 4 filtered out; finished in 0.00s";
        assert!(inventory(good, &names, false).is_ok());
        for bad in [
            good.replace("a ... ok", "a ... ignored, reason"),
            good.replace("a ... ok", "a ... failed"),
            good.replace("a ... ok", "b ... ok"),
            format!("test a ... ok\n{good}"),
            good.replace("test a ... ok\n", ""),
        ] {
            assert!(inventory(&bad, &names, false).is_err());
        }
        assert!(inventory("a: test\na: test", &names, true).is_err());
    }
    #[test]
    fn complete_copy_rejects_foreign_overwrite_concurrent_owner_and_tampering() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap();
        let parent = PathBuf::from(
            std::env::var_os("SLOPSEARX_T07_UNIT_TEST_ROOT")
                .or_else(|| std::env::var_os("SLOPSEARX_TEST_TEMP_ROOT"))
                .expect("registered unit test temp root"),
        );
        for kind in [Kind::Reqwest, Kind::Sdk] {
            let path = parent.join(format!(
                "t07-unit-copy-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            let profile = Profile::open_kind(&root, &path, false, kind).unwrap();
            assert!(profile.verify().is_ok());
            assert!(Profile::open_kind(&root, &path, false, kind).is_err());
            assert!(Profile::open_kind(&root, &path, true, kind).is_err());
            for name in ["src/lib.rs", "Cargo.toml", "Cargo.lock"] {
                let file = profile.package.join(name);
                let raw = fs::read(&file).unwrap();
                fs::write(&file, b"tampered").unwrap();
                assert!(profile.verify().is_err());
                fs::write(&file, raw).unwrap();
            }
            let file = profile.package.join("src/lib.rs");
            let raw = fs::read(&file).unwrap();
            fs::remove_file(&file).unwrap();
            assert!(profile.verify().is_err());
            fs::write(file, raw).unwrap();
            assert!(profile.verify().is_ok());
            drop(profile);
            let restored = Profile::open_kind(&root, &path, true, kind).unwrap();
            drop(restored);
            fs::remove_dir_all(path).unwrap();
        }
    }
    #[cfg(unix)]
    #[test]
    fn output_cap_ack_precedes_original_group_retirement_and_reader_join() {
        use std::os::unix::process::CommandExt;
        let parent = PathBuf::from(
            std::env::var_os("SLOPSEARX_T07_UNIT_TEST_ROOT")
                .or_else(|| std::env::var_os("SLOPSEARX_TEST_TEMP_ROOT"))
                .expect("registered unit test temp root"),
        );
        let path = parent.join(format!(
            "t07-unit-owner-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&path).unwrap();
        let child = Command::new("/bin/sh")
            .args(["-c", "printf ready; exec /bin/cat"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .process_group(0)
            .spawn()
            .unwrap();
        let retained = std::cell::Cell::new(false);
        let mut owned = OwnedChild::new(child, &retained);
        assert!(retained.get());
        let mut output = owned.child.stdout.take().unwrap();
        let mut ack = [0u8; 5];
        output.read_exact(&mut ack).unwrap();
        assert_eq!(&ack, b"ready");
        let (send, receive) = std::sync::mpsc::channel();
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path.join("stdout"))
            .unwrap();
        owned.readers.push(capture(output, file, 4, send));
        owned
            .child
            .stdin
            .as_mut()
            .unwrap()
            .write_all(b"12345")
            .unwrap();
        let doc = document(include_bytes!(
            "../../../docs/rust-rewrite/t07/unit-profile/current-declaration.json"
        ))
        .unwrap();
        assert_eq!(
            receive
                .recv_timeout(Duration::from_secs(
                    doc["command_limits"]["seconds"].as_u64().unwrap()
                ))
                .unwrap(),
            "unit output cap before write"
        );
        assert!(
            owned.child.try_wait().unwrap().is_none(),
            "child must still own its blocked stdin before explicit retirement"
        );
        let error = owned.finish().unwrap_err();
        assert_eq!(error, "unit output cap before write");
        assert_eq!(owned.finish().unwrap_err(), error);
        assert!(!retained.get());
        assert_eq!(owned.probes.len(), 2);
        assert!(owned.retired && owned.group_absent && owned.readers.is_empty());
        assert_eq!(fs::metadata(path.join("stdout")).unwrap().len(), 0);
        drop(owned);
        fs::remove_dir_all(path).unwrap();
    }
}
