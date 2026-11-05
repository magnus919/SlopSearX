//! S01: source-bound advisory identity view for reviewed local third-party forks.
//!
//! cargo-deny 0.20.2 turns a local path package's absent Cargo metadata
//! `source` into no source and skips it before advisory matching. This module
//! produces a clearly labelled, derived metadata view in which only the
//! reviewed local forks carry their upstream registry identity. The physical
//! lock graph and every existing cargo-deny check stay untouched; the derived
//! view is evidence for upstream advisory identity matching only and must never
//! be used as resolver, feature, TLS or shipping evidence.
//!
//! Every mapping row is bound to exact bytes: the physical manifest, the pinned
//! upstream archive, the reviewed patch and the provenance record. A row that
//! does not match exactly one live local package, a live local package that is
//! neither mapped nor inside a declared project-owned root, or any file whose
//! digest drifted fails the whole run closed.
use crate::{Result, command};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs,
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

pub const MAPPINGS: &str = "docs/rust-rewrite/t08a/local-fork-advisory-mappings.json";
pub const SCHEMA: &str = "local-fork-advisory-mappings-v2";
pub const PHYSICAL: &str = "target/rust-evidence/local-fork-advisory-physical-metadata.json";
pub const DERIVED: &str = "target/rust-evidence/local-fork-advisory-metadata.json";
pub const EVIDENCE: &str = "target/rust-evidence/local-fork-advisory.json";
pub const FIXTURES: &str = "docs/rust-rewrite/t08a/fixtures";
pub const FIXTURE_WORK: &str = "target/s01-fixtures";
/// The only upstream identity this adapter can bind today; anything else is a
/// mapping error rather than a silently weaker advisory match.
pub const CRATES_IO: &str = "registry+https://github.com/rust-lang/crates.io-index";

fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn sha_file(path: &Path) -> Result<String> {
    let bytes = fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(digest(&bytes))
}

fn need(ok: bool, why: &str) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(format!("S01 local-fork advisory: {why}"))
    }
}

/// A file the mapping pins by relative path, exact byte count and SHA-256.
#[derive(Clone, Debug)]
pub struct Pinned {
    pub path: String,
    pub bytes: u64,
    pub sha256: String,
}

impl Pinned {
    fn parse(root: &Path, row: &Value, field: &str) -> Result<Self> {
        let value = &row[field];
        let path = value["path"]
            .as_str()
            .ok_or_else(|| format!("S01 mapping {field}.path missing"))?
            .to_owned();
        need(
            !Path::new(&path).is_absolute() && !path.split('/').any(|p| p == ".."),
            &format!("mapping {field}.path escapes the repository: {path}"),
        )?;
        let bytes = value["bytes"]
            .as_u64()
            .ok_or_else(|| format!("S01 mapping {field}.bytes missing"))?;
        let sha256 = value["sha256"]
            .as_str()
            .ok_or_else(|| format!("S01 mapping {field}.sha256 missing"))?
            .to_owned();
        let full = root.join(&path);
        let metadata = fs::symlink_metadata(&full)
            .map_err(|e| format!("S01 mapping {field} {}: {e}", full.display()))?;
        need(
            metadata.file_type().is_file() && metadata.len() == bytes,
            &format!("mapping {field} byte count drifted: {path}"),
        )?;
        need(
            sha_file(&full)? == sha256,
            &format!("mapping {field} digest drifted: {path}"),
        )?;
        Ok(Self {
            path,
            bytes,
            sha256,
        })
    }

    fn evidence(&self) -> Value {
        json!({"path": self.path, "bytes": self.bytes, "sha256": self.sha256})
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    ThirdPartyFork,
    ProjectOwned,
}

#[derive(Clone, Debug)]
pub struct Upstream {
    pub name: String,
    pub version: String,
    pub source: String,
}

/// One reviewed local package classification.
#[derive(Clone, Debug)]
pub struct Row {
    pub kind: Kind,
    pub local_name: String,
    pub local_version: String,
    pub manifest: Pinned,
    pub license: String,
    pub upstream: Option<Upstream>,
    pub archive: Option<Pinned>,
    pub provenance: Option<Pinned>,
    pub patch: Option<Pinned>,
    pub note: String,
}

#[derive(Debug)]
pub struct Mappings {
    pub rows: Vec<Row>,
    pub project_owned_roots: Vec<String>,
    pub sha256: String,
}

fn string_field(row: &Value, field: &str) -> Result<String> {
    row[field]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| format!("S01 mapping {field} missing"))
}

/// Parses the mapping file and verifies every pinned file on disk.
pub fn load_mappings(root: &Path) -> Result<Mappings> {
    let raw = fs::read(root.join(MAPPINGS)).map_err(|e| format!("{MAPPINGS}: {e}"))?;
    let parsed: Value = serde_json::from_slice(&raw).map_err(|e| format!("{MAPPINGS}: {e}"))?;
    need(
        parsed["schema"] == SCHEMA,
        &format!("mapping schema must be {SCHEMA}"),
    )?;
    let roots = parsed["project_owned_roots"]
        .as_array()
        .ok_or("S01 mapping project_owned_roots missing")?
        .iter()
        .map(|value| {
            let root = value
                .as_str()
                .ok_or("S01 project_owned_roots entry must be a string")?;
            need(
                root.ends_with('/')
                    && !root.starts_with('/')
                    && !root.split('/').any(|p| p == ".."),
                &format!("project-owned root must be a relative directory prefix: {root}"),
            )?;
            Ok(root.to_owned())
        })
        .collect::<Result<Vec<_>>>()?;
    need(
        !roots.is_empty(),
        "at least one project-owned root must be declared",
    )?;

    let rows = parsed["mappings"]
        .as_array()
        .ok_or("S01 local-fork advisory: mappings array missing")?;
    let mut parsed_rows = Vec::with_capacity(rows.len());
    let mut seen = BTreeSet::new();
    for row in rows {
        let kind = match row["kind"].as_str().ok_or("S01 mapping kind missing")? {
            "third_party_fork" => Kind::ThirdPartyFork,
            "project_owned" => Kind::ProjectOwned,
            other => return Err(format!("S01 mapping kind is not reviewed: {other}")),
        };
        let local_name = string_field(row, "local_name")?;
        let local_version = string_field(row, "local_version")?;
        need(
            seen.insert((local_name.clone(), local_version.clone())),
            &format!("duplicate mapping for {local_name} {local_version}"),
        )?;
        let manifest = Pinned::parse(root, row, "manifest")?;
        let license = string_field(row, "license")?;
        let note = row["note"].as_str().unwrap_or_default().to_owned();
        let mut entry = Row {
            kind: kind.clone(),
            local_name,
            local_version,
            manifest,
            license,
            upstream: None,
            archive: None,
            provenance: None,
            patch: None,
            note,
        };
        match kind {
            Kind::ProjectOwned => {
                for absent in ["upstream_name", "archive", "provenance", "patch"] {
                    need(
                        row.get(absent).is_none_or(Value::is_null),
                        &format!("project_owned row must not carry {absent}"),
                    )?;
                }
            }
            Kind::ThirdPartyFork => {
                let source = string_field(row, "upstream_source")?;
                need(
                    source == CRATES_IO,
                    &format!("upstream_source must be the crates.io index, found {source}"),
                )?;
                entry.upstream = Some(Upstream {
                    name: string_field(row, "upstream_name")?,
                    version: string_field(row, "upstream_version")?,
                    source,
                });
                entry.archive = Some(Pinned::parse(root, row, "archive")?);
                entry.provenance = Some(Pinned::parse(root, row, "provenance")?);
                entry.patch = Some(Pinned::parse(root, row, "patch")?);
            }
        }
        parsed_rows.push(entry);
    }
    need(!parsed_rows.is_empty(), "no reviewed mappings declared")?;
    for row in &parsed_rows {
        if let (Some(upstream), Some(archive)) = (&row.upstream, &row.archive) {
            let provenance: Value = serde_json::from_slice(
                &fs::read(root.join(&row.provenance.as_ref().expect("fork provenance").path))
                    .map_err(|e| e.to_string())?,
            )
            .map_err(|e| format!("S01 provenance JSON: {e}"))?;
            need(
                provenance["upstream"]["name"] == upstream.name.as_str()
                    && provenance["upstream"]["version"] == upstream.version.as_str()
                    && provenance["upstream"]["archive_sha256"] == archive.sha256.as_str(),
                &format!(
                    "provenance record for {} does not describe the mapped upstream archive",
                    row.local_name
                ),
            )?;
        }
    }
    Ok(Mappings {
        rows: parsed_rows,
        project_owned_roots: roots,
        sha256: digest(&raw),
    })
}

fn relative_manifest(root: &Path, package: &Value) -> Result<String> {
    let manifest = package["manifest_path"]
        .as_str()
        .ok_or("S01 package manifest_path missing")?;
    let canonical = fs::canonicalize(manifest).map_err(|e| format!("{manifest}: {e}"))?;
    let root = fs::canonicalize(root).map_err(|e| e.to_string())?;
    let relative = canonical
        .strip_prefix(&root)
        .map_err(|_| format!("S01 local package manifest escapes the repository: {manifest}"))?;
    relative
        .to_str()
        .map(|value| value.replace('\\', "/"))
        .ok_or_else(|| "S01 package manifest path is not UTF-8".to_owned())
}

fn workspace_members(metadata: &Value) -> Result<BTreeSet<String>> {
    Ok(metadata["workspace_members"]
        .as_array()
        .ok_or("S01 workspace_members missing")?
        .iter()
        .map(|id| id.as_str().map(str::to_owned))
        .collect::<Option<BTreeSet<_>>>()
        .ok_or("S01 workspace member id")?)
}

/// Rewrites only the reviewed local forks to their upstream registry identity.
/// Package IDs and resolve edges stay intact so the derived document keeps the
/// same graph shape; only advisory-relevant identity fields change, and each
/// change is bound to the exact live manifest it was reviewed against.
pub fn transform_metadata(
    root: &Path,
    metadata: &Value,
    mappings: &Mappings,
) -> Result<(Value, Vec<Value>)> {
    let members = workspace_members(metadata)?;
    let mut derived = metadata.clone();
    let mut evidence = Vec::new();
    let mut matched = vec![0usize; mappings.rows.len()];
    let packages = derived["packages"]
        .as_array_mut()
        .ok_or("S01 packages missing")?;
    for package in packages.iter_mut() {
        if package["source"] != Value::Null {
            continue;
        }
        let id = package["id"]
            .as_str()
            .ok_or("S01 package id missing")?
            .to_owned();
        let is_member = members.contains(&id);
        let name = package["name"]
            .as_str()
            .ok_or("S01 package name")?
            .to_owned();
        let version = package["version"]
            .as_str()
            .ok_or("S01 package version")?
            .to_owned();
        let manifest = relative_manifest(root, package)?;
        let license = package["license"].as_str().unwrap_or_default().to_owned();
        let found = mappings
            .rows
            .iter()
            .position(|row| row.local_name == name && row.local_version == version);
        match found {
            Some(index) => {
                let row = &mappings.rows[index];
                matched[index] += 1;
                need(
                    row.manifest.path == manifest,
                    &format!(
                        "mapping for {name} {version} binds manifest {}, live package is {manifest}",
                        row.manifest.path
                    ),
                )?;
                need(
                    row.license == license,
                    &format!(
                        "mapping license for {name} {version} is {}, live package declares {license}",
                        row.license
                    ),
                )?;
                if row.kind == Kind::ProjectOwned {
                    evidence.push(json!({
                        "id": id,
                        "name": name,
                        "version": version,
                        "kind": "project_owned",
                        "workspace_member": is_member,
                        "manifest": row.manifest.evidence(),
                        "license": row.license,
                        "note": row.note,
                    }));
                } else if let Some(upstream) = &row.upstream {
                    package["name"] = json!(upstream.name);
                    package["version"] = json!(upstream.version);
                    package["source"] = json!(upstream.source);
                    evidence.push(json!({
                        "id": id,
                        "local_name": name,
                        "local_version": version,
                        "kind": "third_party_fork",
                        "workspace_member": is_member,
                        "manifest": row.manifest.evidence(),
                        "license": row.license,
                        "upstream_name": upstream.name,
                        "upstream_version": upstream.version,
                        "upstream_source": upstream.source,
                        "archive": row.archive.as_ref().map(Pinned::evidence),
                        "provenance": row.provenance.as_ref().map(Pinned::evidence),
                        "patch": row.patch.as_ref().map(Pinned::evidence),
                        "note": row.note,
                    }));
                } else {
                    return Err(format!(
                        "S01 review row for {name} {version} has no upstream identity"
                    ));
                }
            }
            None => {
                let project_owned = is_member
                    && mappings
                        .project_owned_roots
                        .iter()
                        .any(|root| manifest.starts_with(root));
                need(
                    project_owned,
                    &format!(
                        "local package {name} {version} at {manifest} is neither reviewed nor inside a declared project-owned root"
                    ),
                )?;
                evidence.push(json!({
                    "id": id,
                    "name": name,
                    "version": version,
                    "kind": "project_owned",
                    "workspace_member": true,
                    "manifest": {"path": manifest, "bytes": Value::Null, "sha256": Value::Null},
                    "license": license,
                    "note": "workspace member inside a declared project-owned root; no upstream identity",
                }));
            }
        }
    }
    for (row, count) in mappings.rows.iter().zip(&matched) {
        need(
            *count == 1,
            &format!(
                "mapping for {} {} matched {count} live local packages; exactly one is required",
                row.local_name, row.local_version
            ),
        )?;
    }
    Ok((derived, evidence))
}

fn write_json(root: &Path, relative: &str, value: &Value) -> Result<Vec<u8>> {
    let bytes = serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?;
    let output = root.join(relative);
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(&output, &bytes).map_err(|e| e.to_string())?;
    Ok(bytes)
}

/// Derives the labelled advisory view and retains both metadata documents.
pub fn derive(root: &Path) -> Result<Value> {
    let mappings = load_mappings(root)?;
    let mut records = Vec::new();
    let physical = command(
        root,
        "cargo",
        &["metadata", "--locked", "--format-version", "1"],
        &mut records,
    )?;
    let physical_sha = digest(physical.as_bytes());
    let metadata: Value = serde_json::from_str(&physical).map_err(|e| e.to_string())?;
    let _ = write_json(
        root,
        PHYSICAL,
        &serde_json::from_str(&physical).map_err(|e| e.to_string())?,
    )?;
    let (derived, entries) = transform_metadata(root, &metadata, &mappings)?;
    let derived_bytes = write_json(root, DERIVED, &derived)?;
    Ok(json!({
        "plan": "S01",
        "role": "local-fork-advisory-derived-metadata",
        "mappings": MAPPINGS,
        "mappings_sha256": mappings.sha256,
        "project_owned_roots": mappings.project_owned_roots,
        "physical_metadata_path": PHYSICAL,
        "physical_metadata_sha256": physical_sha,
        "derived_metadata_path": DERIVED,
        "derived_metadata_sha256": digest(&derived_bytes),
        "derived_metadata_bytes": derived_bytes.len(),
        "entries": entries,
        "reviewed_forks": entries.iter().filter(|e| e["kind"] == "third_party_fork").count(),
        "scope": "upstream advisory identity view only; not resolver, feature, TLS or shipping evidence",
    }))
}

fn cargo_home() -> PathBuf {
    match std::env::var("CARGO_HOME") {
        Ok(value) if !value.is_empty() => PathBuf::from(value),
        _ => PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".cargo"),
    }
}

/// Locates the fetched advisory database repository below `db-path` and records
/// its exact revision and last commit time.
fn advisory_db_revision(db_root: &Path) -> Result<Value> {
    let mut candidates = Vec::new();
    if let Ok(entries) = fs::read_dir(db_root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.join(".git").exists() {
                candidates.push(path);
            }
        }
    }
    // Prefer the most recently fetched repository so a stale cache entry in the
    // same CARGO_HOME cannot label a fresh run.
    candidates.sort_by_key(|path| {
        fs::metadata(path.join(".git"))
            .and_then(|metadata| metadata.modified())
            .ok()
    });
    let repository = candidates.last().ok_or_else(|| {
        format!(
            "S01 advisory database was not fetched below {}",
            db_root.display()
        )
    })?;
    let revision = git_output(repository, &["rev-parse", "HEAD"])?;
    let committed = git_output(repository, &["log", "-1", "--format=%cI"])?;
    Ok(json!({
        "path": repository.to_string_lossy(),
        "revision": revision.trim(),
        "committed_at": committed.trim(),
    }))
}

fn git_output(repository: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repository)
        .args(args)
        .output()
        .map_err(|e| format!("git {}: {e}", args.join(" ")))?;
    need(
        output.status.success(),
        &format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        ),
    )?;
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

struct Run {
    stdout: String,
    stderr: String,
    exit_code: Option<i32>,
}

fn run_capture(program: &str, args: &[&str], cwd: &Path, env: &[(&str, String)]) -> Result<Run> {
    let mut child = Command::new(program);
    child
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (key, value) in env {
        child.env(key, value);
    }
    let output = child
        .output()
        .map_err(|e| format!("{program} {}: {e}", args.join(" ")))?;
    Ok(Run {
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        exit_code: output.status.code(),
    })
}

/// Runs the pinned checker. `SLOPSEARX_CARGO_DENY` may point at an exact
/// cargo-deny binary; otherwise the installed `cargo deny` subcommand is used.
fn deny(root: &Path, args: &[&str]) -> Result<Run> {
    match std::env::var("SLOPSEARX_CARGO_DENY") {
        Ok(binary) if !binary.is_empty() => run_capture(&binary, args, root, &[]),
        _ => {
            let mut all = vec!["deny"];
            all.extend_from_slice(args);
            run_capture("cargo", &all, root, &[])
        }
    }
}

fn deny_version(root: &Path) -> Result<String> {
    let run = deny(root, &["--version"])?;
    need(run.exit_code == Some(0), "cargo-deny --version failed")?;
    Ok(run.stdout.trim().to_owned())
}

/// Parses cargo-deny JSON diagnostics and keeps only advisory findings.
fn advisory_findings(stderr: &str) -> Vec<Value> {
    let mut findings = Vec::new();
    for line in stderr.lines() {
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let Some(advisory) = value["fields"]["advisory"].as_object() else {
            continue;
        };
        let span = value["fields"]["labels"]
            .as_array()
            .and_then(|labels| labels.first())
            .and_then(|label| label["span"].as_str())
            .unwrap_or_default();
        let mut tokens = span.split_whitespace();
        findings.push(json!({
            "id": advisory.get("id").cloned().unwrap_or(Value::Null),
            "package": advisory.get("package").cloned().unwrap_or(Value::Null),
            "title": advisory.get("title").cloned().unwrap_or(Value::Null),
            "severity": value["fields"]["severity"].clone(),
            "code": value["fields"]["code"].clone(),
            "matched_package": tokens.next(),
            "matched_version": tokens.next(),
            "matched_span": span,
        }));
    }
    findings
}

fn sort_findings(findings: &mut [Value]) {
    findings.sort_by(|a, b| {
        (a["id"].as_str(), a["matched_span"].as_str())
            .cmp(&(b["id"].as_str(), b["matched_span"].as_str()))
    });
}

/// Runs the pinned checker against the derived view using a fresh advisory
/// database and the repository's own advisory configuration.
pub fn check(root: &Path) -> Result<Value> {
    let version = deny_version(root)?;
    let db_root = cargo_home().join("advisory-dbs");
    let run = deny(
        root,
        &[
            "--locked",
            "--workspace",
            "--metadata-path",
            DERIVED,
            "--config",
            "deny.toml",
            "--format",
            "json",
            "check",
            "advisories",
        ],
    )?;
    let mut findings = advisory_findings(&run.stderr);
    sort_findings(&mut findings);
    let database = advisory_db_revision(&db_root)?;
    let evidence = json!({
        "tool": "cargo-deny",
        "tool_version": version,
        "database": database,
        "metadata_path": DERIVED,
        "config": "deny.toml",
        "command": "cargo deny --locked --workspace --metadata-path <derived> --config deny.toml --format json check advisories",
        "exit_code": run.exit_code,
        "findings": findings,
    });
    need(
        run.exit_code == Some(0) && findings.is_empty(),
        &format!(
            "the derived upstream advisory view reported {} finding(s) with exit {:?}",
            findings.len(),
            run.exit_code
        ),
    )?;
    Ok(evidence)
}

/// Deterministic local advisory fixtures exercised through the real checker.
pub fn fixtures(root: &Path) -> Result<Value> {
    let work = root.join(FIXTURE_WORK);
    if work.exists() {
        fs::remove_dir_all(&work).map_err(|e| e.to_string())?;
    }
    fs::create_dir_all(&work).map_err(|e| e.to_string())?;
    let mut cases = Vec::new();
    let mut repositories = Vec::new();
    for name in ["advisory-db", "advisory-db-clean"] {
        let source = root.join(FIXTURES).join(name);
        let destination = work.join(name);
        copy_tree(&source, &destination)?;
        let commit = commit_fixture(&destination)?;
        repositories.push(json!({"name": name, "commit": commit}));
    }
    let port = free_port()?;
    let daemon = start_daemon(&work, port)?;
    let result = fixture_cases(root, &work, port, &mut cases);
    let _ = daemon_kill(daemon);
    let cases = result?;
    Ok(json!({
        "plan": "S01",
        "role": "local-fork-advisory-fixtures",
        "tool_version": deny_version(root)?,
        "advisory_databases": repositories,
        "daemon": {"url": format!("git://localhost:{port}"), "base_path": work.to_string_lossy()},
        "cases": cases,
    }))
}

fn copy_tree(source: &Path, destination: &Path) -> Result<()> {
    fs::create_dir_all(destination).map_err(|e| e.to_string())?;
    for entry in fs::read_dir(source).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let target = destination.join(entry.file_name());
        if entry.file_type().map_err(|e| e.to_string())?.is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), &target).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// Creates the fixture advisory repository with a byte-identical, date-pinned
/// commit so the database revision is reproducible.
fn commit_fixture(directory: &Path) -> Result<String> {
    let fixed = "2020-01-01T00:00:00+0000".to_owned();
    let env = [
        ("GIT_AUTHOR_DATE", fixed.clone()),
        ("GIT_COMMITTER_DATE", fixed.clone()),
        ("GIT_AUTHOR_NAME", "S01 fixture".to_owned()),
        ("GIT_AUTHOR_EMAIL", "s01-fixture@example.invalid".to_owned()),
        ("GIT_COMMITTER_NAME", "S01 fixture".to_owned()),
        (
            "GIT_COMMITTER_EMAIL",
            "s01-fixture@example.invalid".to_owned(),
        ),
    ];
    let init = run_capture("git", &["init", "-q", "-b", "main"], directory, &env)?;
    need(
        init.exit_code == Some(0),
        "git init for fixture database failed",
    )?;
    let add = run_capture("git", &["add", "-A"], directory, &env)?;
    need(
        add.exit_code == Some(0),
        "git add for fixture database failed",
    )?;
    let commit = run_capture(
        "git",
        &["commit", "-q", "-m", "S01 fixture advisory database"],
        directory,
        &env,
    )?;
    need(
        commit.exit_code == Some(0),
        "git commit for fixture database failed",
    )?;
    Ok(git_output(directory, &["rev-parse", "HEAD"])?
        .trim()
        .to_owned())
}

fn free_port() -> Result<u16> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    drop(listener);
    Ok(port)
}

fn start_daemon(work: &Path, port: u16) -> Result<Child> {
    let port_arg = format!("--port={port}");
    let base_arg = format!("--base-path={}", work.display());
    let child = Command::new("git")
        .args([
            "daemon",
            "--export-all",
            "--reuseaddr",
            "--listen=127.0.0.1",
            port_arg.as_str(),
            base_arg.as_str(),
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("git daemon: {e}"))?;
    let deadline = Instant::now() + Duration::from_secs(15);
    while Instant::now() < deadline {
        if TcpStream::connect(("127.0.0.1", port)).is_ok() {
            return Ok(child);
        }
        thread::sleep(Duration::from_millis(50));
    }
    let mut child = child;
    let _ = child.kill();
    Err(format!("git daemon did not start on port {port}"))
}

fn daemon_kill(mut child: Child) -> Result<()> {
    let _ = child.kill();
    let _ = child.wait();
    Ok(())
}

struct Fixture<'a> {
    name: &'a str,
    database: &'a str,
    metadata: &'a str,
    expectation: Value,
}

fn fixture_run(
    root: &Path,
    work: &Path,
    port: u16,
    cases: &mut Vec<Value>,
    fixture: Fixture<'_>,
) -> Result<()> {
    let Fixture {
        name,
        database,
        metadata,
        expectation,
    } = fixture;
    let db_cache = work.join("db-cache");
    let config = work.join(format!("{name}.toml"));
    fs::write(
        &config,
        format!(
            "[advisories]\ndb-urls = [\"git://localhost:{port}/{database}\"]\ndb-path = \"{}\"\n",
            db_cache.display()
        ),
    )
    .map_err(|e| e.to_string())?;
    let run = deny(
        root,
        &[
            "--locked",
            "--workspace",
            "--metadata-path",
            metadata,
            "--config",
            config.to_str().ok_or("fixture config path is not UTF-8")?,
            "--format",
            "json",
            "check",
            "advisories",
        ],
    )?;
    let mut findings = advisory_findings(&run.stderr);
    sort_findings(&mut findings);
    cases.push(json!({
        "case": name,
        "database": database,
        "metadata": metadata,
        "expectation": expectation,
        "exit_code": run.exit_code,
        "findings": findings,
    }));
    Ok(())
}

fn fixture_cases(
    root: &Path,
    work: &Path,
    port: u16,
    cases: &mut Vec<Value>,
) -> Result<Vec<Value>> {
    // 1. The raw physical graph omits the local `libxml` fork entirely.
    fixture_run(
        root,
        work,
        port,
        cases,
        Fixture {
            name: "raw-physical",
            database: "advisory-db",
            metadata: PHYSICAL,
            expectation: json!({"advisories": [
                {"id": "RUSTSEC-2020-9002", "package": "hyper", "count": 1},
                {"id": "RUSTSEC-2020-9004", "package": "process-wrap", "count": 1}
            ]}),
        },
    )?;
    // 2. The derived view adds both canonical-name and renamed local forks, and
    //    the exact-version fixture proves the matched version, not just a name.
    fixture_run(
        root,
        work,
        port,
        cases,
        Fixture {
            name: "derived-view",
            database: "advisory-db",
            metadata: DERIVED,
            expectation: json!({"advisories": [
                {"id": "RUSTSEC-2020-9001", "package": "libxml", "version": "0.3.22", "count": 1},
                {"id": "RUSTSEC-2020-9002", "package": "hyper", "version": "1.12.0", "count": 2},
                {"id": "RUSTSEC-2020-9004", "package": "process-wrap", "version": "10.0.1", "count": 2}
            ], "absent": ["RUSTSEC-2020-9003"]}),
        },
    )?;
    // 3. A database with no relevant advisory must exit cleanly with no finding.
    fixture_run(
        root,
        work,
        port,
        cases,
        Fixture {
            name: "clean-database",
            database: "advisory-db-clean",
            metadata: DERIVED,
            expectation: json!({"advisories": [], "absent": ["RUSTSEC-2020-9001"]}),
        },
    )?;
    verify_cases(cases)?;
    Ok(std::mem::take(cases))
}

fn verify_cases(cases: &[Value]) -> Result<()> {
    for case in cases {
        let name = case["case"].as_str().unwrap_or_default();
        let findings = case["findings"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or_default();
        let expected = case["expectation"]["advisories"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or_default();
        let expected_ids: BTreeSet<&str> = expected
            .iter()
            .filter_map(|entry| entry["id"].as_str())
            .collect();
        let actual_ids: BTreeSet<&str> = findings
            .iter()
            .filter_map(|entry| entry["id"].as_str())
            .collect();
        need(
            actual_ids == expected_ids,
            &format!("fixture {name} matched {actual_ids:?}; expected exactly {expected_ids:?}"),
        )?;
        for entry in expected {
            let id = entry["id"].as_str().unwrap_or_default();
            let count = findings
                .iter()
                .filter(|finding| finding["id"] == id)
                .count();
            need(
                count == entry["count"].as_u64().unwrap_or_default() as usize,
                &format!(
                    "fixture {name} matched {id} {count} time(s); expected {}",
                    entry["count"]
                ),
            )?;
            for finding in findings.iter().filter(|finding| finding["id"] == id) {
                need(
                    finding["package"] == entry["package"],
                    &format!("fixture {name} advisory {id} package drifted"),
                )?;
                if let Some(version) = entry["version"].as_str() {
                    need(
                        finding["matched_version"] == version
                            && finding["matched_package"] == entry["package"],
                        &format!(
                            "fixture {name} advisory {id} did not match {} {version}",
                            entry["package"]
                        ),
                    )?;
                }
            }
        }
        if let Some(absent) = case["expectation"]["absent"].as_array() {
            for id in absent {
                need(
                    !actual_ids.contains(id.as_str().unwrap_or_default()),
                    &format!("fixture {name} unexpectedly matched {id}"),
                )?;
            }
        }
        let clean = case["case"].as_str() == Some("clean-database");
        need(
            (case["exit_code"].as_i64() == Some(0)) == clean && (clean == findings.is_empty()),
            &format!(
                "fixture {name} exit code {:?} disagrees with its findings",
                case["exit_code"]
            ),
        )?;
    }
    Ok(())
}

/// Derives the advisory view, runs the pinned checker and writes the retained
/// evidence record.
pub fn run(root: &Path) -> Result<Value> {
    let derived = derive(root)?;
    let checked = check(root)?;
    let evidence = json!({
        "plan": "S01",
        "role": "local-fork-advisory",
        "derived": derived,
        "check": checked,
        "scope": "upstream advisory identity view only; not resolver, feature, TLS or shipping evidence",
    });
    write_json(root, EVIDENCE, &evidence)?;
    Ok(evidence)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repository() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    #[test]
    fn reviewed_mapping_covers_every_live_local_package() {
        let root = repository();
        let mappings = load_mappings(&root).expect("mapping loads");
        assert_eq!(mappings.rows.len(), 4);
        let kinds: Vec<_> = mappings.rows.iter().map(|row| row.kind.clone()).collect();
        assert_eq!(
            kinds,
            vec![
                Kind::ThirdPartyFork,
                Kind::ThirdPartyFork,
                Kind::ThirdPartyFork,
                Kind::ProjectOwned
            ]
        );
        assert!(mappings.rows.iter().all(|row| {
            row.manifest.path.ends_with("Cargo.toml")
                && row.manifest.bytes > 0
                && row.manifest.sha256.len() == 64
        }));
    }

    #[test]
    fn mapping_rejects_a_drifted_pinned_file() {
        let root = repository();
        let mut mappings = load_mappings(&root).expect("mapping loads");
        mappings.rows[0].archive.as_mut().expect("archive").sha256 = "0".repeat(64);
        let error = Pinned::parse(
            &root,
            &json!({"archive": {"path": mappings.rows[0].archive.as_ref().expect("archive").path, "bytes": 1, "sha256": "0".repeat(64)}}),
            "archive",
        )
        .expect_err("drifted archive must be rejected");
        assert!(error.contains("byte count drifted") || error.contains("digest drifted"));
    }

    #[test]
    fn transform_rejects_an_unreviewed_vendor_package() {
        let root = repository();
        let mappings = load_mappings(&root).expect("mapping loads");
        let metadata = json!({
            "workspace_members": [],
            "packages": [
                {"id": "path+file:///repo/vendor/expat#0.1.0", "name": "new-fork",
                 "version": "0.1.0", "source": null, "license": "MIT",
                 "manifest_path": root.join("vendor/expat/Cargo.toml")}
            ]
        });
        let error = transform_metadata(&root, &metadata, &mappings).expect_err("must reject");
        assert!(error.contains("neither reviewed nor inside a declared project-owned root"));
    }

    #[test]
    fn transform_rejects_an_unused_mapping_row() {
        let root = repository();
        let mappings = load_mappings(&root).expect("mapping loads");
        let metadata = json!({"workspace_members": [], "packages": []});
        let error = transform_metadata(&root, &metadata, &mappings).expect_err("must reject");
        assert!(error.contains("matched 0 live local packages"));
    }

    #[test]
    fn transform_rewrites_only_reviewed_forks_and_keeps_ids() {
        let root = repository();
        let mappings = load_mappings(&root).expect("mapping loads");
        let manifest = root.join("vendor/ssx-cli-hyper/Cargo.toml");
        let metadata = json!({
            "workspace_members": ["path+file:///repo/vendor/ssx-cli-hyper#1.12.0"],
            "packages": [{
                "id": "path+file:///repo/vendor/ssx-cli-hyper#1.12.0",
                "name": "ssx-cli-hyper",
                "version": "1.12.0",
                "source": null,
                "license": "MIT",
                "manifest_path": manifest,
            }]
        });
        let mut mappings = mappings;
        mappings
            .rows
            .retain(|row| row.local_name == "ssx-cli-hyper");
        let (derived, evidence) =
            transform_metadata(&root, &metadata, &mappings).expect("transform");
        assert_eq!(derived["packages"][0]["name"], "hyper");
        assert_eq!(derived["packages"][0]["version"], "1.12.0");
        assert_eq!(derived["packages"][0]["source"], CRATES_IO);
        assert_eq!(
            derived["packages"][0]["id"],
            "path+file:///repo/vendor/ssx-cli-hyper#1.12.0"
        );
        assert_eq!(evidence[0]["kind"], "third_party_fork");
    }

    #[test]
    fn advisory_findings_extract_id_package_and_version() {
        let line = json!({
            "type": "diagnostic",
            "fields": {
                "severity": "error",
                "message": "fixture",
                "code": "vulnerability",
                "labels": [{"message": "security vulnerability detected",
                            "span": "libxml 0.3.22 path+file:///repo/vendor", "line": 1, "column": 1}],
                "advisory": {"id": "RUSTSEC-2020-9001", "package": "libxml"}
            }
        });
        let findings = advisory_findings(&line.to_string());
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0]["id"], "RUSTSEC-2020-9001");
        assert_eq!(findings[0]["matched_package"], "libxml");
        assert_eq!(findings[0]["matched_version"], "0.3.22");
    }
}
