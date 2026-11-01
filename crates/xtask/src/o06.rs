//! Native, additive CI successors; never publishes reviews or executes target PR code.
use crate::{Result, command};
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    fs,
    io::Read,
    path::Path,
    process::{Command, Stdio},
    time::{Duration, Instant},
};
const HARNESS: &str = "groktopus/codereview";
const REVISION: &str = "1c76d40290cdaf9b6603721f899da6c7e8a382b4";
const PROTECTED_PINS: [(&str, &str); 4] = [
    (
        ".github/workflows/pr-publish.yml",
        "e68453f8a89ed6fa0c918a1e5ecadd41f8b7d03a",
    ),
    (
        ".github/workflows/pr-review-analysis.yml",
        "57cebed7b7f4e73e30ca673c88d8c4d56c149c6b",
    ),
    (
        ".github/workflows/protected-source-context-probe.yml",
        "768063de420395c234bbbb5b87af47b33485c8d9",
    ),
    (
        ".github/pr-review-publisher-policy.json",
        "701803af2a510fc127cdaa59056776f1d439ab6a",
    ),
];
fn bounded(path: &Path) -> Result<String> {
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(4 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    ensure(bytes.len() <= 4 * 1024 * 1024, "CI input exceeds 4 MiB")?;
    String::from_utf8(bytes).map_err(|e| e.to_string())
}
fn load_json(path: &Path) -> Result<Value> {
    serde_json::from_str(&bounded(path)?).map_err(|e| format!("{}: {e}", path.display()))
}
fn parse_yaml(raw: &str) -> Result<Value> {
    let options = serde_saphyr::options! {
        duplicate_keys: serde_saphyr::DuplicateKeyPolicy::Error,
        budget: serde_saphyr::budget! {max_events: 100_000, max_aliases: 0, max_anchors: 0, flow_nesting_limit: 64},
    };
    serde_saphyr::from_str_with_options(raw, options).map_err(|e| e.to_string())
}
fn ensure(ok: bool, reason: &str) -> Result<()> {
    if ok { Ok(()) } else { Err(reason.into()) }
}
fn yaml(path: &Path) -> Result<Value> {
    parse_yaml(&bounded(path)?).map_err(|e| format!("{}: {e}", path.display()))
}
fn object<'a>(v: &'a Value, label: &str) -> Result<&'a serde_json::Map<String, Value>> {
    v.as_object()
        .ok_or_else(|| format!("{label} must be an object"))
}
pub fn inventory(root: &Path) -> Result<Value> {
    let mut gates = serde_json::Map::new();
    for entry in fs::read_dir(root.join(".github/workflows")).map_err(|e| e.to_string())? {
        let path = entry.map_err(|e| e.to_string())?.path();
        if !matches!(
            path.extension().and_then(|p| p.to_str()),
            Some("yml" | "yaml")
        ) {
            continue;
        }
        let w = yaml(&path)?;
        for (id, job) in object(&w["jobs"], "workflow jobs")? {
            ensure(
                job.get("uses").is_some() || job["steps"].as_array().is_some_and(|s| !s.is_empty()),
                "gate has no executable step",
            )?;
            gates.insert(
                format!(
                    "{}:{id}",
                    path.file_name()
                        .and_then(|p| p.to_str())
                        .ok_or("workflow name")?
                ),
                gate_source(&w, job)?,
            );
        }
    }
    Ok(Value::Object(gates))
}
fn gate_source(workflow: &Value, job: &Value) -> Result<Value> {
    let mut context = object(workflow, "workflow")?.clone();
    context.remove("jobs");
    Ok(json!({"workflow":context,"job":job}))
}
fn validate_gate_source(source: &Value, mapping: &Value) -> Result<()> {
    ensure(
        mapping["source"] == *source,
        "executable workflow/job/step controls differ from successor ledger",
    )
}
fn validate_policy(p: &Value) -> Result<()> {
    ensure(
        p["schema"] == "pr-review-protected-publication-policy.v5"
            && p["repository"]["name"] == "magnus919/SlopSearX"
            && p["repository"]["id"] == 1263452316_u64
            && p["default_branch"] == "main"
            && p["called_harness"]["repository"] == HARNESS
            && p["called_harness"]["sha"] == REVISION
            && p["called_harness"]["path"] == ".github/workflows/pr-analysis.yml"
            && p["credential"]["kind"] == "ACTIONS_TOKEN"
            && p["artifact_trust_mode"] == "API_BOUND_SHA256"
            && p["artifact_download_mode"] == "AUTHENTICATED_API_LOCATION_NO_AUTH",
        "protected policy identity/provenance mismatch",
    )
}
fn permissions(p: &Value, writer: bool) -> Result<()> {
    ensure(
        *p == json!({"actions":"read","contents":"read","pull-requests":if writer {"write"} else {"read"}}),
        "publisher permissions differ from accepted least privilege",
    )
}
fn protected(root: &Path) -> Result<()> {
    validate_policy(&load_json(
        &root.join(".github/pr-review-publisher-policy.json"),
    )?)?;
    let p = yaml(&root.join(".github/workflows/pr-publish.yml"))?;
    permissions(&p["permissions"], false)?;
    permissions(&p["jobs"]["publisher"]["permissions"], true)?;
    ensure(
        p["on"]["workflow_run"]["workflows"] == json!(["PR Review Analysis"])
            && p["on"]["workflow_run"]["types"] == json!(["completed"]),
        "publisher trigger mismatch",
    )?;
    let job = &p["jobs"]["publisher"];
    ensure(
        job["environment"]["name"] == "code-review-publication"
            && job["steps"][0]["with"]["ref"] == "${{ github.sha }}"
            && job["steps"][0]["with"]["persist-credentials"] == false,
        "protected source/environment boundary mismatch",
    )?;
    let pins = load_json(&root.join("docs/rust-rewrite/ci/protected-sources.json"))?;
    let expected: BTreeSet<_> = PROTECTED_PINS.iter().map(|(p, _)| *p).collect();
    ensure(
        object(&pins, "pins")?
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>()
            == expected,
        "protected pin coverage incomplete",
    )?;
    for (path, pin) in object(&pins, "pins")? {
        let hash = command(root, "git", &["hash-object", "--", path], &mut vec![])?;
        let accepted = PROTECTED_PINS
            .iter()
            .find(|(p, _)| p == path)
            .ok_or("unaccepted protected path")?
            .1;
        ensure(
            pin.as_str() == Some(accepted) && hash.trim() == accepted,
            "protected source changed: trust review and repinning required",
        )?;
    }
    Ok(())
}
fn mirror(raw: &[u8]) -> Result<Value> {
    let mut v: Value = if raw.iter().all(u8::is_ascii_whitespace) {
        json!({})
    } else {
        serde_json::from_slice(raw).map_err(|e| e.to_string())?
    };
    let mirrors = v
        .as_object_mut()
        .ok_or("Docker config must be an object")?
        .entry("registry-mirrors")
        .or_insert(json!([]))
        .as_array_mut()
        .ok_or("mirrors must be an array")?;
    ensure(
        mirrors
            .iter()
            .all(|v| v.as_str().is_some_and(|s| !s.is_empty())),
        "invalid mirror entry",
    )?;
    if !mirrors.iter().any(|v| v == "https://mirror.gcr.io") {
        mirrors.push(json!("https://mirror.gcr.io"));
    }
    Ok(v)
}
fn digest(s: &str) -> bool {
    s.strip_prefix("sha256:").is_some_and(|s| {
        s.len() == 64
            && s.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}
fn manifest(v: &Value) -> Result<()> {
    ensure(
        v["schemaVersion"] == 2
            && matches!(
                v["mediaType"].as_str(),
                Some(
                    "application/vnd.oci.image.index.v1+json"
                        | "application/vnd.docker.distribution.manifest.list.v2+json"
                )
            ),
        "expected OCI index",
    )?;
    let rows = v["manifests"].as_array().ok_or("missing manifests")?;
    for arch in ["amd64", "arm64"] {
        ensure(
            rows.iter()
                .filter(|m| m["platform"]["os"] == "linux" && m["platform"]["architecture"] == arch)
                .count()
                == 1,
            "missing/ambiguous Linux platform",
        )?;
    }
    ensure(
        rows.iter()
            .all(|m| m["digest"].as_str().is_some_and(digest)),
        "invalid immutable manifest digest",
    )
}
fn bootstrap(root: &Path, p: &Value, dest: &Path) -> Result<Value> {
    validate_policy(p)?;
    ensure(
        dest.is_absolute()
            && dest
                .file_name()
                .is_some_and(|s| s == "trusted-review-harness"),
        "destination must be absolute trusted-review-harness",
    )?;
    ensure(
        fs::symlink_metadata(dest).is_err(),
        "destination already exists",
    )?;
    let parent = dest
        .parent()
        .ok_or("destination parent")?
        .canonicalize()
        .map_err(|e| e.to_string())?;
    ensure(
        !parent.starts_with(root.canonicalize().map_err(|e| e.to_string())?),
        "harness must be outside target source",
    )?;
    Ok(
        json!({"repository":HARNESS,"sha":REVISION,"destination":parent.join("trusted-review-harness"),"execute_target_code":false}),
    )
}
struct GitGroup(u32);
impl Drop for GitGroup {
    fn drop(&mut self) {
        #[cfg(unix)]
        {
            let _ = Command::new("/bin/kill")
                .args(["-KILL", "--", &format!("-{}", self.0)])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
        #[cfg(windows)]
        {
            let _ = Command::new("taskkill")
                .args(["/PID", &self.0.to_string(), "/T", "/F"])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
    }
}
fn git_bounded(program: &Path, cwd: &Path, args: &[&str], seconds: u64) -> Result<String> {
    let mut process = Command::new(program);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        process.process_group(0);
    }
    process
        .current_dir(cwd)
        .args(args)
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let mut child = process.spawn().map_err(|e| e.to_string())?;
    let _group = GitGroup(child.id());
    let stdout = child.stdout.take().ok_or("git stdout")?;
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let result = stdout
            .take(8193)
            .read_to_end(&mut bytes)
            .map(|_| bytes)
            .map_err(|e| e.to_string());
        let _ = tx.send(result);
    });
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            ensure(status.success(), "trusted git command failed")?;
            let bytes = rx
                .recv_timeout(Duration::from_secs(seconds).saturating_sub(start.elapsed()))
                .map_err(|_| "trusted git output timed out")??;
            ensure(bytes.len() <= 8192, "git output too large")?;
            return String::from_utf8(bytes).map_err(|e| e.to_string());
        }
        if start.elapsed() >= Duration::from_secs(seconds) {
            let _ = child.kill();
            let _ = child.wait();
            return Err("trusted git command timed out".into());
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}
fn checkout_with(
    root: &Path,
    p: &Value,
    dest: &Path,
    mut run: impl FnMut(&Path, &[&str], u64) -> Result<String>,
) -> Result<()> {
    let plan = bootstrap(root, p, dest)?;
    let destination = plan["destination"].as_str().ok_or("destination encoding")?;
    let parent = Path::new(destination)
        .parent()
        .ok_or("destination parent")?;
    run(
        parent,
        &[
            "-c",
            "core.hooksPath=/dev/null",
            "-c",
            "credential.helper=",
            "clone",
            "--quiet",
            "--no-checkout",
            "--filter=blob:none",
            &format!("https://github.com/{HARNESS}.git"),
            destination,
        ],
        120,
    )?;
    run(
        Path::new(destination),
        &[
            "-c",
            "core.hooksPath=/dev/null",
            "checkout",
            "--quiet",
            "--detach",
            REVISION,
        ],
        30,
    )?;
    ensure(
        run(Path::new(destination), &["rev-parse", "HEAD"], 10)?.trim() == REVISION,
        "trusted harness revision mismatch",
    )
}
// Exact reviewed source assets: binary formats are never treated as UTF8 source.
const A05_ASSET_MANIFEST: &str = "docs/rust-rewrite/a05/source-assets.json";
const A05_ASSET_MANIFEST_SHA256: &str =
    "a04b55d47739d0caab79a25df573346850594849baa5e9265071f758a8053afa";
const A05_MAX_ASSET_BYTES: u64 = 5_869_813;
// Finite CPython Unicode16 source provenance; no directory-prefix exemption.
const F05_PYTHON_TEXT_ASSET_PATHS: [&str; 8] = [
    "contracts/unicode/python-text-source/Modules___sre__sre.c.txt",
    "contracts/unicode/python-text-source/Include__cpython__unicodeobject.h.txt",
    "contracts/unicode/python-text-source/Objects__unicodectype.c.txt",
    "contracts/unicode/python-text-source/Objects__unicodetype_db.h.txt",
    "contracts/unicode/python-text-source/Objects__unicodeobject.c.txt",
    "contracts/unicode/python-text-source/Include__patchlevel.h.txt",
    "contracts/unicode/python-text-source/Modules__unicodedata_db.h.txt",
    "contracts/unicode/python-text-source/LICENSE.txt",
];
#[derive(Clone, Copy, Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct AssetTextRules {
    preserve_cr: bool,
    preserve_final_newline: bool,
    preserve_trailing: bool,
    license_separator: bool,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceAsset {
    path: String,
    bytes: u64,
    sha256: String,
    encoding: String,
    kind: String,
    provenance: String,
    text_rules: AssetTextRules,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceAssetManifest {
    schema_version: u64,
    assets: Vec<SourceAsset>,
}
#[derive(Clone, Copy)]
struct AssetPolicy<'a> {
    path: &'a str,
    digest: &'a str,
    count: usize,
}
fn asset_digest(raw: &[u8]) -> String {
    use sha2::{Digest as _, Sha256};
    Sha256::digest(raw)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
fn source_assets(
    root: &Path,
    policy: AssetPolicy<'_>,
) -> Result<std::collections::BTreeMap<String, SourceAsset>> {
    let raw = bounded(&root.join(policy.path))?;
    ensure(
        asset_digest(raw.as_bytes()) == policy.digest,
        "source asset manifest digest drift",
    )?;
    let manifest: SourceAssetManifest = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
    ensure(
        manifest.schema_version == 1 && manifest.assets.len() == policy.count,
        "source asset manifest shape",
    )?;
    let mut assets = std::collections::BTreeMap::new();
    for asset in manifest.assets {
        ensure(
            !asset
                .path
                .split('/')
                .any(|part| matches!(part, "" | "." | ".."))
                && (asset.path.starts_with("crates/ssx-source-codecs/")
                    || asset.path
                        == "crates/ssx-domain/tests/fixtures/a05_previous_media_sanitizer.rs"
                    || F05_PYTHON_TEXT_ASSET_PATHS.contains(&asset.path.as_str())),
            "invalid source asset path",
        )?;
        ensure(
            asset.bytes > 0
                && asset.bytes <= A05_MAX_ASSET_BYTES
                && asset.sha256.len() == 64
                && asset.sha256.bytes().all(|byte| byte.is_ascii_hexdigit()),
            "invalid source asset size/digest",
        )?;
        ensure(
            matches!(asset.encoding.as_str(), "binary" | "utf8"),
            "unknown source asset encoding",
        )?;
        ensure(
            (asset.encoding == "binary")
                == matches!(
                    asset.kind.as_str(),
                    "compressed native source oracle" | "native mapping/validity bytes"
                ),
            "asset kind/encoding mismatch",
        )?;
        ensure(
            matches!(
                asset.kind.as_str(),
                "compressed native source oracle"
                    | "native mapping/validity bytes"
                    | "generated Rust table"
                    | "source JSON observation"
                    | "frozen prior source fixture"
                    | "licensed upstream source/notice"
            ) && !asset.provenance.is_empty(),
            "unknown source asset kind/provenance",
        )?;
        ensure(
            asset.kind != "generated Rust table"
                || asset.path == "crates/ssx-source-codecs/src/unicode_names.rs",
            "unreviewed oversized generated source",
        )?;
        ensure(
            !asset.path.ends_with(".rs")
                || asset.bytes <= 500 * 1024
                || (asset.kind == "generated Rust table"
                    && asset.path == "crates/ssx-source-codecs/src/unicode_names.rs"),
            "unreviewed oversized Rust source",
        )?;
        let rules = asset.text_rules;
        ensure(
            !rules.license_separator
                || asset.path == "crates/ssx-source-codecs/LICENSE-PYTHON"
                || asset.path == "contracts/unicode/python-text-source/LICENSE.txt",
            "unreviewed license separator",
        )?;
        ensure(
            asset.encoding != "binary"
                || !(rules.preserve_cr
                    || rules.preserve_final_newline
                    || rules.preserve_trailing
                    || rules.license_separator),
            "binary asset has text waivers",
        )?;
        ensure(
            assets.insert(asset.path.clone(), asset).is_none(),
            "duplicate source asset path",
        )?;
    }
    Ok(assets)
}
fn read_source_asset(root: &Path, asset: &SourceAsset) -> Result<Option<String>> {
    use sha2::{Digest as _, Sha256};
    let path = root.join(&asset.path);
    let metadata = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
    ensure(
        metadata.file_type().is_file() && metadata.len() == asset.bytes,
        "source asset file/length drift",
    )?;
    let maximum = asset
        .bytes
        .checked_add(1)
        .ok_or("source asset size overflow")?;
    let mut reader = fs::File::open(&path)
        .map_err(|e| e.to_string())?
        .take(maximum);
    // Only UTF8 assets need retained storage for semantic validation. Binary
    // mappings/oracles stream through fixed scratch. No read exceeds length+1.
    let mut text = if asset.encoding == "utf8" {
        Some(Vec::with_capacity(
            usize::try_from(asset.bytes).map_err(|_| "source asset size")?,
        ))
    } else {
        None
    };
    let mut scratch = [0u8; 16 * 1024];
    let mut count = 0u64;
    let mut digest = Sha256::new();
    loop {
        let n = reader.read(&mut scratch).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        count = count
            .checked_add(n as u64)
            .ok_or("source asset read overflow")?;
        ensure(count <= asset.bytes, "source asset grew while reading")?;
        digest.update(&scratch[..n]);
        if let Some(text) = &mut text {
            text.extend_from_slice(&scratch[..n]);
        }
    }
    ensure(
        count == asset.bytes
            && digest
                .finalize()
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
                == asset.sha256,
        "source asset length/digest drift",
    )?;
    text.map(|bytes| String::from_utf8(bytes).map_err(|e| e.to_string()))
        .transpose()
}

fn hygiene_text(path: &str, raw: &str) -> Result<()> {
    hygiene_text_with_rules(path, raw, AssetTextRules::default())
}
fn hygiene_text_with_rules(path: &str, raw: &str, rules: AssetTextRules) -> Result<()> {
    use sha2::{Digest as _, Sha256};
    // Exact durable output intentionally has no text newline. Keep JSON and
    // all other hygiene validation; no other path/bytes receive this exception.
    let durable_fixture = path == "crates/xtask/fixtures/f13/native-record.json"
        && Sha256::digest(raw.as_bytes())
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
            == "fbf5c2b453e4c4e4fdd54b5c372be642d036e5cf2db788de0df963e24e0ba203";
    ensure(
        rules.preserve_cr || !raw.contains('\r'),
        "mixed/CR line endings",
    )?;
    ensure(
        rules.preserve_final_newline
            || durable_fixture
            || raw.is_empty()
            || (raw.ends_with('\n') && !raw.ends_with("\n\n")),
        "missing or repeated final newline",
    )?;
    for line in raw.lines() {
        ensure(
            rules.preserve_trailing || line.trim_end_matches([' ', '\t']) == line,
            "trailing whitespace",
        )?;
        ensure(
            !["<<<<<<< ", "=======", ">>>>>>> "]
                .iter()
                .any(|m| line.starts_with(m))
                || (rules.license_separator && line.starts_with("=======")),
            "merge conflict marker",
        )?;
    }
    if path.ends_with(".json") {
        let _: Value = serde_json::from_str(raw).map_err(|e| e.to_string())?;
    }
    if path.ends_with(".yaml") || path.ends_with(".yml") {
        parse_yaml(raw)?;
    }
    Ok(())
}
fn hygiene(root: &Path) -> Result<()> {
    hygiene_with_policy(
        root,
        AssetPolicy {
            path: A05_ASSET_MANIFEST,
            digest: A05_ASSET_MANIFEST_SHA256,
            count: 208,
        },
    )
}
fn hygiene_with_policy(root: &Path, policy: AssetPolicy<'_>) -> Result<()> {
    let assets = source_assets(root, policy)?;
    let mut seen = BTreeSet::new();
    let files = command(
        root,
        "git",
        &[
            "ls-files",
            "--cached",
            "--others",
            "--exclude-standard",
            "-z",
        ],
        &mut vec![],
    )?;
    for file in files.split('\0').filter(|p| !p.is_empty()).filter(|p| {
        assets.contains_key(*p)
            || p.starts_with("crates/")
            || p.starts_with(".github/workflows/")
            || p.starts_with("docs/rust-rewrite/ci/")
            || matches!(
                *p,
                "Cargo.toml" | "Cargo.lock" | "rust-toolchain.toml" | "deny.toml"
            )
    }) {
        if let Some(asset) = assets.get(file) {
            seen.insert(file.to_string());
            if let Some(raw) = read_source_asset(root, asset).map_err(|e| format!("{file}: {e}"))? {
                hygiene_text_with_rules(file, &raw, asset.text_rules)
                    .map_err(|e| format!("{file}: {e}"))?;
            }
        } else {
            ensure(
                !file.ends_with(".bin") && !file.ends_with(".zlib") && !file.ends_with(".gz"),
                "unlisted binary source asset",
            )?;
            let raw = bounded(&root.join(file))?;
            ensure(
                raw.len() <= 500 * 1024,
                "new native gate source exceeds 500 KiB",
            )?;
            hygiene_text(file, &raw).map_err(|e| format!("{file}: {e}"))?;
        }
    }
    ensure(
        seen.len() == assets.len(),
        "declared source asset absent from source inventory",
    )
}
pub fn helper(root: &Path, args: &[String]) -> Result<()> {
    match args.first().map(String::as_str) {
        Some("source-hygiene") if args.len()==1 => hygiene(root),
        Some("review-bootstrap") if args.len()==2 => checkout_with(root,&load_json(&root.join(".github/pr-review-publisher-policy.json"))?,Path::new(&args[1]),|cwd,args,seconds|git_bounded(Path::new("git"),cwd,args,seconds)),
        Some("inventory") if args.len()==1 => {println!("{}",serde_json::to_string_pretty(&inventory(root)?).map_err(|e|e.to_string())?);Ok(())}
        Some("mirror-config") if args.len()==2 => {println!("{}",serde_json::to_string_pretty(&mirror(&fs::read(&args[1]).map_err(|e|e.to_string())?)?).map_err(|e|e.to_string())?);Ok(())}
        Some("manifest") if args.len()==2 => manifest(&load_json(Path::new(&args[1]))?),
        Some("review-bootstrap-plan") if args.len()==2 => {println!("{}",bootstrap(root,&load_json(&root.join(".github/pr-review-publisher-policy.json"))?,Path::new(&args[1]))?);Ok(())}
        _ => Err("usage: cargo xtask ci-helper {inventory|source-hygiene|mirror-config FILE|manifest FILE|review-bootstrap-plan ABS_DEST|review-bootstrap ABS_DEST}".into())
    }
}
pub fn check(root: &Path, records: &mut Vec<Value>) -> Result<()> {
    hygiene(root)?;
    let actual = inventory(root)?;
    let ledger = load_json(&root.join("docs/rust-rewrite/ci/gates.json"))?;
    let gates = object(&ledger["gates"], "gate ledger")?;
    ensure(
        object(&actual, "actual gates")?
            .keys()
            .collect::<BTreeSet<_>>()
            == gates.keys().collect::<BTreeSet<_>>(),
        "workflow gate coverage differs",
    )?;
    for (id, source) in object(&actual, "actual gates")? {
        let g = &gates[id];
        validate_gate_source(source, g).map_err(|e| format!("{id}: {e}"))?;
        ensure(
            g["successor"].as_str().is_some_and(|s| !s.is_empty())
                && g["owner"].as_str().is_some_and(|s| !s.is_empty())
                && matches!(
                    g["state"].as_str(),
                    Some("additive" | "pending-owner" | "retained-external")
                ),
            "missing successor decision",
        )?;
    }
    ensure(
        ledger["precommit_source"] == yaml(&root.join(".pre-commit-config.yaml"))?,
        "precommit drift",
    )?;
    let hooks = yaml(&root.join(".pre-commit-config.yaml"))?;
    let mut actual_hooks = BTreeSet::new();
    for repo in hooks["repos"].as_array().ok_or("missing hook repos")? {
        for hook in repo["hooks"].as_array().ok_or("missing hooks")? {
            actual_hooks.insert(hook["id"].as_str().ok_or("missing hook id")?);
        }
    }
    let mapped = object(&ledger["precommit_successors"], "hook successors")?;
    ensure(
        actual_hooks == mapped.keys().map(String::as_str).collect(),
        "incomplete hook successor coverage",
    )?;
    ensure(
        mapped
            .values()
            .all(|v| v.as_str().is_some_and(|s| !s.is_empty())),
        "empty hook successor",
    )?;
    ensure(
        ledger["pr_template_source"].as_str()
            == Some(
                fs::read_to_string(root.join(".github/PULL_REQUEST_TEMPLATE.md"))
                    .map_err(|e| e.to_string())?
                    .as_str(),
            ),
        "PR template drift",
    )?;
    protected(root)?;
    command(
        root,
        "cargo",
        &["test", "--locked", "-p", "xtask", "o06::tests::"],
        records,
    )?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    struct AssetFixture(std::path::PathBuf);
    impl AssetFixture {
        fn new() -> Self {
            use std::sync::atomic::{AtomicU64, Ordering};
            static NEXT: AtomicU64 = AtomicU64::new(0);
            for _ in 0..64 {
                let root = std::env::temp_dir().join(format!(
                    "ssx-o06-assets-{}-{}",
                    std::process::id(),
                    NEXT.fetch_add(1, Ordering::Relaxed)
                ));
                if fs::create_dir(&root).is_ok() {
                    assert!(
                        Command::new("git")
                            .args(["init", "--quiet"])
                            .current_dir(&root)
                            .status()
                            .unwrap()
                            .success()
                    );
                    return Self(root);
                }
            }
            panic!("cannot create exclusive owned source fixture")
        }
        fn write(&self, path: &str, raw: &[u8]) {
            let path = self.0.join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, raw).unwrap();
        }
        fn check(&self, assets: Vec<Value>) -> Result<()> {
            let count = assets.len();
            let raw = serde_json::to_string(&json!({"schema_version":1,"assets":assets})).unwrap();
            self.write(A05_ASSET_MANIFEST, raw.as_bytes());
            let digest = asset_digest(raw.as_bytes());
            // Same real hygiene entry; only its private trusted manifest pin/count
            // are supplied by this owned fixture. Production has no such option.
            hygiene_with_policy(
                &self.0,
                AssetPolicy {
                    path: A05_ASSET_MANIFEST,
                    digest: &digest,
                    count,
                },
            )
        }
    }
    impl Drop for AssetFixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).expect("only owned asset fixture");
        }
    }
    fn test_asset(path: &str, raw: &[u8], binary: bool) -> Value {
        json!({"path":path,"bytes":raw.len(),"sha256":asset_digest(raw),"encoding":if binary {"binary"} else {"utf8"},"kind":if binary {"native mapping/validity bytes"} else {"licensed upstream source/notice"},"provenance":"owned test data","text_rules":{"preserve_cr":false,"preserve_final_newline":false,"preserve_trailing":false,"license_separator":false}})
    }
    #[test]
    fn pinned_source_assets_reject_unknown_duplicate_traversal_and_absent_declarations() {
        let fixture = AssetFixture::new();
        let path = "crates/ssx-source-codecs/tests/probe.bin";
        let raw = [0xff, 0xfe];
        fixture.write(path, &raw);
        let asset = test_asset(path, &raw, true);
        fixture.check(vec![asset.clone()]).unwrap();
        assert!(
            fixture
                .check(vec![asset.clone(), asset.clone()])
                .unwrap_err()
                .contains("duplicate")
        );
        for (key, value) in [
            ("kind", json!("unknown")),
            ("encoding", json!("utf16")),
            ("unknown_policy", json!(true)),
            ("path", json!("crates/ssx-source-codecs/../escape.bin")),
            ("path", json!("/crates/ssx-source-codecs/tests/probe.bin")),
        ] {
            let mut bad = asset.clone();
            bad[key] = value;
            assert!(fixture.check(vec![bad]).is_err(), "{key}");
        }
        let mut bad = asset.clone();
        bad["text_rules"]["unreviewed"] = json!(true);
        assert!(fixture.check(vec![bad]).is_err());
        let mut bad = asset.clone();
        bad["text_rules"]["preserve_trailing"] = json!(true);
        assert!(fixture.check(vec![bad]).is_err());
        let mut absent = asset;
        absent["path"] = json!("crates/ssx-source-codecs/tests/absent.bin");
        assert!(fixture.check(vec![absent]).is_err());
    }
    #[test]
    fn actual_source_hygiene_rejects_changed_assets_and_unlisted_binary_or_oversized_source() {
        let fixture = AssetFixture::new();
        let path = "crates/ssx-source-codecs/tests/probe.bin";
        let raw = [0xff, 0xfe];
        fixture.write(path, &raw);
        let asset = test_asset(path, &raw, true);
        fixture.write(path, &[0xff, 0xfd]);
        assert!(
            fixture
                .check(vec![asset.clone()])
                .unwrap_err()
                .contains("digest")
        );
        fixture.write(path, &[0xff]);
        assert!(
            fixture
                .check(vec![asset.clone()])
                .unwrap_err()
                .contains("length")
        );
        fixture.write(path, &raw);
        let ordinary = "crates/plain.rs";
        for (raw, reason) in [
            ("<<<<<<< branch\n", "merge conflict"),
            ("value \n", "trailing"),
            ("value\r\n", "line endings"),
        ] {
            fixture.write(ordinary, raw.as_bytes());
            assert!(
                fixture
                    .check(vec![asset.clone()])
                    .unwrap_err()
                    .contains(reason)
            );
        }
        fixture.write(ordinary, &vec![b'x'; 500 * 1024 + 1]);
        assert!(
            fixture
                .check(vec![asset.clone()])
                .unwrap_err()
                .contains("500 KiB")
        );
        fs::remove_file(fixture.0.join(ordinary)).unwrap();
        fixture.write("crates/new.bin", b"ASCII binary\n");
        assert!(
            fixture
                .check(vec![asset])
                .unwrap_err()
                .contains("unlisted binary")
        );
    }
    #[test]
    fn pinned_text_keeps_semantic_validation_and_only_named_whitespace_waivers() {
        let fixture = AssetFixture::new();
        for (path, raw) in [
            ("crates/ssx-source-codecs/tests/bad.json", "{\n"),
            ("crates/ssx-source-codecs/tests/bad.yaml", "x: 1\nx: 2\n"),
        ] {
            fixture.write(path, raw.as_bytes());
            assert!(
                fixture
                    .check(vec![test_asset(path, raw.as_bytes(), false)])
                    .is_err()
            );
            fs::remove_file(fixture.0.join(path)).unwrap();
        }
        let path = "crates/ssx-source-codecs/LICENSE-PYTHON";
        let raw = b"=======\n";
        fixture.write(path, raw);
        let mut asset = test_asset(path, raw, false);
        assert!(fixture.check(vec![asset.clone()]).is_err());
        asset["text_rules"]["license_separator"] = json!(true);
        fixture.check(vec![asset]).unwrap();
        let raw = b"<<<<<<< branch\n";
        fixture.write(path, raw);
        let mut asset = test_asset(path, raw, false);
        asset["text_rules"]["license_separator"] = json!(true);
        assert!(fixture.check(vec![asset]).is_err());
        fs::remove_file(fixture.0.join(path)).unwrap();
        let path = "crates/ssx-domain/tests/fixtures/a05_previous_media_sanitizer.rs";
        let raw = b"source without newline";
        fixture.write(path, raw);
        let mut asset = test_asset(path, raw, false);
        asset["text_rules"]["preserve_final_newline"] = json!(true);
        fixture.check(vec![asset]).unwrap();
        let raw = b"source with trailing ";
        fixture.write(path, raw);
        let mut asset = test_asset(path, raw, false);
        asset["text_rules"]["preserve_final_newline"] = json!(true);
        assert!(fixture.check(vec![asset]).is_err());
    }
    #[test]
    fn only_reviewed_generated_rust_path_gets_size_allowance_with_full_hygiene() {
        let fixture = AssetFixture::new();
        let path = "crates/ssx-source-codecs/src/unicode_names.rs";
        let raw = "//x\n".repeat(130_000);
        fixture.write(path, raw.as_bytes());
        let mut asset = test_asset(path, raw.as_bytes(), false);
        asset["kind"] = json!("generated Rust table");
        fixture.check(vec![asset]).unwrap();
        let raw = raw + "trailing \n";
        fixture.write(path, raw.as_bytes());
        let mut asset = test_asset(path, raw.as_bytes(), false);
        asset["kind"] = json!("generated Rust table");
        assert!(fixture.check(vec![asset]).unwrap_err().contains("trailing"));
        let mut asset = test_asset(
            "crates/ssx-source-codecs/src/unreviewed.rs",
            b"source\n",
            false,
        );
        asset["kind"] = json!("generated Rust table");
        assert!(fixture.check(vec![asset]).is_err());
    }
    #[test]
    fn ordinary_f13_exact_byte_exception_and_manifest_digest_guard_remain() {
        let fixture = AssetFixture::new();
        let path = "crates/ssx-source-codecs/tests/probe.bin";
        let raw = [0xff, 0xfe];
        fixture.write(path, &raw);
        let asset = test_asset(path, &raw, true);
        fixture.write(
            "crates/xtask/fixtures/f13/native-record.json",
            include_bytes!("../fixtures/f13/native-record.json"),
        );
        fixture.check(vec![asset]).unwrap();
        let bad = AssetPolicy {
            path: A05_ASSET_MANIFEST,
            digest: "0000000000000000000000000000000000000000000000000000000000000000",
            count: 1,
        };
        assert!(
            hygiene_with_policy(&fixture.0, bad)
                .unwrap_err()
                .contains("manifest digest")
        );
    }
    #[test]
    #[cfg(unix)]
    fn w08_current_workflow_producer_and_actual_aggregate_list_fail_closed() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let workflow = yaml(&root.join(".github/workflows/rust.yml")).unwrap();
        let ledger = load_json(&root.join("docs/rust-rewrite/ci/gates.json")).unwrap();
        let runtime = &workflow["jobs"]["foundation-runtime"];
        let source = gate_source(&workflow, runtime).unwrap();
        validate_gate_source(&source, &ledger["gates"]["rust.yml:foundation-runtime"]).unwrap();
        let steps = runtime["steps"].as_array().unwrap();
        let command = "cargo xtask check W08 --output target/rust-evidence/W08.json";
        assert_eq!(
            steps.iter().filter(|step| step["run"] == command).count(),
            1
        );
        let mut missing = source.clone();
        missing["job"]["steps"]
            .as_array_mut()
            .unwrap()
            .retain(|step| step["run"] != command);
        assert_eq!(
            validate_gate_source(&missing, &ledger["gates"]["rust.yml:foundation-runtime"])
                .unwrap_err(),
            "executable workflow/job/step controls differ from successor ledger"
        );
        let foundation = &workflow["jobs"]["foundation"];
        validate_gate_source(
            &gate_source(&workflow, foundation).unwrap(),
            &ledger["gates"]["rust.yml:foundation"],
        )
        .unwrap();
        let steps = foundation["steps"].as_array().unwrap();
        assert_eq!(
            steps
                .iter()
                .filter(
                    |step| step["with"]["path"].as_str().is_some_and(|paths| paths
                        .lines()
                        .any(|line| line == "target/rust-evidence/W08.json"))
                )
                .count(),
            1
        );
        let loops = steps
            .iter()
            .filter_map(|step| step["run"].as_str())
            .flat_map(str::lines)
            .filter(|line| line.trim_start().starts_with("for plan "))
            .collect::<Vec<_>>();
        assert_eq!(loops.len(), 1);
        let actual = loops[0].trim();
        let names = actual
            .strip_prefix("for plan ")
            .unwrap()
            .strip_suffix("; do test -s \"target/rust-evidence/$plan.json\"; done")
            .unwrap()
            .split_whitespace()
            .collect::<Vec<_>>();
        assert!(names.contains(&"W08"));
        assert!(
            names
                .iter()
                .all(|name| !name.is_empty()
                    && name.bytes().all(|byte| byte.is_ascii_alphanumeric()))
        );
        assert_eq!(
            names.iter().copied().collect::<BTreeSet<_>>().len(),
            names.len()
        );
        struct Owned(std::path::PathBuf);
        impl Drop for Owned {
            fn drop(&mut self) {
                let _ = fs::remove_dir_all(&self.0);
            }
        }
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("ssx-w08-aggregate-{}-{nonce}", std::process::id()));
        fs::create_dir(&path).unwrap();
        let owned = Owned(path);
        let evidence = owned.0.join("target/rust-evidence");
        fs::create_dir_all(&evidence).unwrap();
        // Every name comes from the actual current job's exact loop, not a second list.
        // These nonempty owned fixture files prove the guard, not W08 runtime behavior.
        for name in &names {
            fs::write(
                evidence.join(format!("{name}.json")),
                b"owned guard fixture\n",
            )
            .unwrap();
        }
        // The existing bounded owner deliberately discards stderr. A finite wrapper
        // returns the exact guard's exit code on stdout without changing that guard.
        let wrapper = r#"set +e; /bin/bash -euo pipefail -c "$1"; code=$?; printf '%s' "$code""#;
        let guard = || {
            git_bounded(
                Path::new("/bin/bash"),
                &owned.0,
                &[
                    "-euo",
                    "pipefail",
                    "-c",
                    wrapper,
                    "w08-aggregate-control",
                    actual,
                ],
                5,
            )
            .unwrap()
        };
        assert_eq!(
            guard(),
            "0",
            "complete actual aggregate list must pass first"
        );
        fs::remove_file(evidence.join("W08.json")).unwrap();
        assert_eq!(
            guard(),
            "1",
            "sole missing W08.json must fail the actual aggregate guard"
        );
        for name in names.iter().filter(|name| **name != "W08") {
            assert!(evidence.join(format!("{name}.json")).is_file());
        }
    }
    #[test]
    fn gate_ledger_rejects_failure_weakening_and_all_execution_control_drift() {
        let original = json!({"on":{"pull_request":{}},"permissions":{"contents":"read"},"concurrency":{"group":"gate"},"jobs":{"test":{"runs-on":"ubuntu-24.04","needs":["build"],"strategy":{"fail-fast":true},"timeout-minutes":10,"environment":"test","steps":[{"run":"cargo test --locked","timeout-minutes":5}]}}});
        let source = gate_source(&original, &original["jobs"]["test"]).unwrap();
        let mapping = json!({"source":source});
        validate_gate_source(&source, &mapping).unwrap();
        for (pointer, value) in [
            ("/jobs/test/continue-on-error", json!(true)),
            ("/jobs/test/steps/0/continue-on-error", json!(true)),
            ("/jobs/test/steps/0/timeout-minutes", json!(1)),
            ("/jobs/test/timeout-minutes", json!(1)),
            ("/jobs/test/needs", json!([])),
            ("/jobs/test/strategy/fail-fast", json!(false)),
            ("/jobs/test/environment", json!("unprotected")),
            ("/permissions/contents", json!("write")),
            ("/on", json!({"workflow_dispatch":{}})),
            ("/concurrency/group", json!("other")),
        ] {
            let mut changed = original.clone();
            let (parent, key) = pointer.rsplit_once('/').unwrap();
            changed
                .pointer_mut(parent)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .insert(key.into(), value);
            let changed_source = gate_source(&changed, &changed["jobs"]["test"]).unwrap();
            assert!(
                validate_gate_source(&changed_source, &mapping).is_err(),
                "accepted drift at {pointer}"
            );
        }
    }
    #[test]
    #[cfg(unix)]
    fn subprocess_environment_failure_output_and_deadline_are_bounded() {
        let cwd = std::env::temp_dir();
        let output=git_bounded(Path::new("/bin/sh"),&cwd,&["-c","printf '%s/%s/%s' \"${GITHUB_TOKEN-unset}\" \"$GIT_CONFIG_GLOBAL\" \"$GIT_TERMINAL_PROMPT\""],2).unwrap();
        assert_eq!(output, "unset//dev/null/0");
        assert!(git_bounded(Path::new("/bin/sh"), &cwd, &["-c", "exit 3"], 2).is_err());
        let start = Instant::now();
        assert!(git_bounded(Path::new("/bin/sh"), &cwd, &["-c", "exec sleep 30"], 1).is_err());
        assert!(start.elapsed() < Duration::from_secs(5));
        assert!(
            git_bounded(
                Path::new("/bin/sh"),
                &cwd,
                &["-c", "head -c 9000 /dev/zero"],
                2
            )
            .is_err()
        );
    }
    #[test]
    fn yaml_and_hygiene_fail_closed_on_ambiguous_or_malformed_sources() {
        for bad in [
            "permissions: read\npermissions: write\n",
            "x: &a [1]\ny: *a\n",
            "a: [\n",
        ] {
            assert!(parse_yaml(bad).is_err());
        }
        for bad in [
            "value \n",
            "value\r\n",
            "no final newline",
            "value\n\n",
            "<<<<<<< branch\n",
            "=======\n",
        ] {
            assert!(hygiene_text("source.rs", bad).is_err());
        }
        hygiene_text("source.rs", "fn main() {}\n").unwrap();
        let fixture = include_str!("../fixtures/f13/native-record.json");
        let path = "crates/xtask/fixtures/f13/native-record.json";
        hygiene_text(path, fixture).unwrap();
        assert!(hygiene_text("other.json", fixture).is_err());
        assert!(hygiene_text(path, &fixture.replace("Title0", "changed")).is_err());
        assert!(hygiene_text(path, "{").is_err());
        assert!(hygiene_text("source.json", "{\n").is_err());
    }
    #[test]
    fn bootstrap_executes_only_immutable_checkout_and_checks_actual_head() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let p = load_json(&root.join(".github/pr-review-publisher-policy.json")).unwrap();
        let temp = std::env::temp_dir().join(format!("o06-bootstrap-{}", std::process::id()));
        fs::create_dir_all(&temp).unwrap();
        let dest = temp.join("trusted-review-harness");
        let mut calls = Vec::new();
        checkout_with(&root, &p, &dest, |cwd, args, seconds| {
            calls.push((
                cwd.to_path_buf(),
                args.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
                seconds,
            ));
            Ok(if args == ["rev-parse", "HEAD"] {
                format!("{REVISION}\n")
            } else {
                String::new()
            })
        })
        .unwrap();
        assert_eq!(calls.len(), 3);
        assert_eq!(calls[0].2, 120);
        assert_eq!(calls[1].2, 30);
        assert_eq!(calls[2].2, 10);
        assert!(
            calls[0]
                .1
                .contains(&format!("https://github.com/{HARNESS}.git"))
        );
        assert!(calls[0].1.contains(&"--no-checkout".to_owned()));
        assert!(calls[1].1.contains(&REVISION.to_owned()));
        assert!(checkout_with(&root, &p, &dest, |_, _, _| Ok("attacker revision".into())).is_err());
        assert!(checkout_with(&root, &p, &dest, |_, _, _| Err("clone failed".into())).is_err());
        fs::create_dir_all(&dest).unwrap();
        assert!(bootstrap(&root, &p, &dest).is_err());
        fs::remove_dir_all(temp).unwrap();
    }
    #[test]
    fn protected_workflow_mutation_cannot_self_authorize_by_repinning() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let temp = std::env::temp_dir().join(format!("o06-trust-{}", std::process::id()));
        fs::create_dir_all(temp.join(".github/workflows")).unwrap();
        fs::create_dir_all(temp.join("docs/rust-rewrite/ci")).unwrap();
        for (path, _) in PROTECTED_PINS {
            fs::copy(root.join(path), temp.join(path)).unwrap();
        }
        let pins = load_json(&root.join("docs/rust-rewrite/ci/protected-sources.json")).unwrap();
        fs::write(
            temp.join("docs/rust-rewrite/ci/protected-sources.json"),
            serde_json::to_vec(&pins).unwrap(),
        )
        .unwrap();
        protected(&temp).unwrap();
        let path = ".github/workflows/pr-publish.yml";
        let original = fs::read_to_string(root.join(path)).unwrap();
        for (before, after) in [
            (
                "github.event.workflow_run.event == 'pull_request_target'",
                "true",
            ),
            (
                "ref: ${{ github.sha }}",
                "ref: ${{ github.event.workflow_run.head_sha }}",
            ),
            ("pull-requests: write", "contents: write"),
            (REVISION, "main"),
            ("persist-credentials: false", "persist-credentials: true"),
        ] {
            let changed = original.replace(before, after);
            assert_ne!(changed, original);
            fs::write(temp.join(path), changed).unwrap();
            let new_hash =
                command(&temp, "git", &["hash-object", "--", path], &mut vec![]).unwrap();
            let mut repinned = pins.clone();
            repinned[path] = json!(new_hash.trim());
            fs::write(
                temp.join("docs/rust-rewrite/ci/protected-sources.json"),
                serde_json::to_vec(&repinned).unwrap(),
            )
            .unwrap();
            assert!(protected(&temp).is_err(), "self-repin allowed {before}");
        }
        fs::remove_dir_all(temp).unwrap();
    }
    #[test]
    fn mirror_preserves_and_is_idempotent() {
        let v = mirror(br#"{"debug":true,"registry-mirrors":["https://private"]}"#).unwrap();
        assert_eq!(v["debug"], true);
        assert_eq!(
            v["registry-mirrors"],
            json!(["https://private", "https://mirror.gcr.io"])
        );
        assert_eq!(mirror(&serde_json::to_vec(&v).unwrap()).unwrap(), v);
        for bad in [
            "[]",
            "null",
            "{",
            r#"{"registry-mirrors":"url"}"#,
            r#"{"registry-mirrors":[null]}"#,
        ] {
            assert!(mirror(bad.as_bytes()).is_err());
        }
    }
    #[test]
    fn manifest_rejects_missing_duplicate_and_mutable_platforms() {
        let d = format!("sha256:{}", "a".repeat(64));
        let good = json!({"schemaVersion":2,"mediaType":"application/vnd.oci.image.index.v1+json","manifests":[{"digest":d,"platform":{"os":"linux","architecture":"amd64"}},{"digest":d,"platform":{"os":"linux","architecture":"arm64"}}]});
        manifest(&good).unwrap();
        for pointer in [
            "/manifests/0/digest",
            "/manifests/1/platform/architecture",
            "/mediaType",
        ] {
            let mut bad = good.clone();
            *bad.pointer_mut(pointer).unwrap() = json!("amd64");
            assert!(manifest(&bad).is_err());
        }
        assert!(!digest("sha256:abc"));
        assert!(!digest(&format!("sha256:{}", "A".repeat(64))));
    }
    #[test]
    fn permissions_cannot_expand() {
        permissions(
            &json!({"actions":"read","contents":"read","pull-requests":"write"}),
            true,
        )
        .unwrap();
        for bad in [
            json!("write-all"),
            json!({"actions":"write","contents":"read","pull-requests":"write"}),
            json!({"actions":"read","contents":"write","pull-requests":"write"}),
            json!({"actions":"read","contents":"read","pull-requests":"write","id-token":"write"}),
        ] {
            assert!(permissions(&bad, true).is_err());
        }
    }
    #[test]
    fn bootstrap_rejects_untrusted_identity_and_target_source() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let p = load_json(&root.join(".github/pr-review-publisher-policy.json")).unwrap();
        validate_policy(&p).unwrap();
        for (pointer, value) in [
            ("/called_harness/sha", json!("main")),
            ("/called_harness/repository", json!("evil/review")),
            ("/repository/id", json!(1)),
            ("/credential/kind", json!("APP_TOKEN")),
            ("/artifact_trust_mode", json!("UNVERIFIED")),
        ] {
            let mut bad = p.clone();
            *bad.pointer_mut(pointer).unwrap() = value;
            assert!(validate_policy(&bad).is_err());
        }
        assert!(bootstrap(&root, &p, &root.join("trusted-review-harness")).is_err());
        assert!(bootstrap(&root, &p, Path::new("relative/trusted-review-harness")).is_err());
    }
}
