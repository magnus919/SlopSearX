//! T08A standalone native client: source-bound real child, never Python/backend.
pub(crate) use crate::cli_source_custody as source_custody;
#[path = "t08a_process_wrap.rs"]
mod process_wrap_guard;
#[path = "t08a_tls_roles.rs"]
pub(crate) mod tls_roles;
use crate::{Result, command, t08a_child};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};
const BINDINGS: &str = include_str!("../../../docs/rust-rewrite/t08a/bindings.json");
const PRIVATE_LIB_LINT_ARGS: &[&str] = &[
    "clippy",
    "--locked",
    "-p",
    "ssx-cli-hyper",
    "--features",
    "full,source-client-framing",
    "--lib",
    "--",
    "-D",
    "warnings",
];
const PRIVATE_FULL_UNIT_ARGS: &[&str] = &[
    "test",
    "--locked",
    "-p",
    "ssx-cli-hyper",
    "--features",
    "full,source-client-framing",
    "--lib",
    "--",
    "--test-threads=1",
];
const PRIVATE_LIB_ROLE_ERROR: &str = "T08A private LIB Clippy role missing, duplicate or failed";
const PRIVATE_UNIT_ROLE_ERROR: &str = "T08A full private unit role missing, duplicate or failed";
fn private_role_positions(records: &[Value]) -> Result<[usize; 2]> {
    let mut positions = [0; 2];
    for (index, (args, reason)) in [
        (PRIVATE_LIB_LINT_ARGS, PRIVATE_LIB_ROLE_ERROR),
        (PRIVATE_FULL_UNIT_ARGS, PRIVATE_UNIT_ROLE_ERROR),
    ]
    .into_iter()
    .enumerate()
    {
        let expected_args = json!(args);
        let mut selected = records
            .iter()
            .enumerate()
            .filter(|(_, record)| record["program"] == "cargo" && record["args"] == expected_args);
        let (position, record) = selected.next().ok_or_else(|| reason.to_owned())?;
        ensure(
            selected.next().is_none() && record["exit_code"] == 0,
            reason,
        )?;
        positions[index] = position;
    }
    let stdout = records[positions[1]]["stdout"]
        .as_str()
        .ok_or(PRIVATE_UNIT_ROLE_ERROR)?;
    for name in include_str!("../../../docs/rust-rewrite/t08a/private-http-controls.txt").lines() {
        let passed = format!("test {name} ... ok");
        ensure(
            stdout
                .lines()
                .filter(|line| *line == passed.as_str())
                .count()
                == 1,
            "T08A private HTTP control did not execute exactly once",
        )?;
    }
    Ok(positions)
}
fn private_qualification_controls(records: &mut Vec<Value>) -> Result<()> {
    let successful = records.clone();
    // Real successful LIB + full-test records pass FIRST. Each control removes
    // one actual record; no synthetic successful producer transcript is made.
    let positions = private_role_positions(&successful)?;
    let positive_sha256 = hash(&serde_json::to_vec(&successful).map_err(|e| e.to_string())?);
    let mut controls = Vec::new();
    for (role, position, reason) in [
        ("private-lib", positions[0], PRIVATE_LIB_ROLE_ERROR),
        ("private-full-unit", positions[1], PRIVATE_UNIT_ROLE_ERROR),
    ] {
        let mut omitted = successful.clone();
        let removed = omitted.remove(position);
        let rest = successful
            .iter()
            .enumerate()
            .filter(|(index, _)| *index != position)
            .map(|(_, record)| record.clone())
            .collect::<Vec<_>>();
        ensure(
            omitted == rest,
            "T08A role control changed another actual record",
        )?;
        let rejection = private_role_positions(&omitted).err();
        ensure(
            rejection.as_deref() == Some(reason),
            "T08A omitted role was not rejected for its exact reason",
        )?;
        controls.push(json!({"role":role,"removed_actual_record_index":position,
            "removed_actual_record_sha256":hash(&serde_json::to_vec(&removed).map_err(|e|e.to_string())?),
            "mutant_sha256":hash(&serde_json::to_vec(&omitted).map_err(|e|e.to_string())?),
            "exact_rejection":reason,"all_remaining_actual_records_equal":true}));
    }
    records.push(json!({"plan":"T08A","private_lint_unit_roles":{
        "positive_actual_transcript_sha256":positive_sha256,"positive_validated_before_mutants":true,
        "actual_private_lib_record":positions[0],"actual_full_unit_record":positions[1],
        "omission_controls":controls,"scope":"actual private LIB strict lint and unfiltered full unit test; no shipping feature authority"}}));
    Ok(())
}

fn hash(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn ensure(ok: bool, msg: &str) -> Result<()> {
    if ok { Ok(()) } else { Err(msg.into()) }
}
fn sources(root: &Path) -> Result<Value> {
    let embedded: Value = serde_json::from_str(BINDINGS).map_err(|e| e.to_string())?;
    let current =
        fs::read(root.join("docs/rust-rewrite/t08a/bindings.json")).map_err(|e| e.to_string())?;
    ensure(
        current == BINDINGS.as_bytes(),
        "T08A runtime bindings differ from compiled gate",
    )?;
    for row in embedded["files"]
        .as_array()
        .ok_or("T08A finite source inventory missing")?
    {
        let path = row["path"].as_str().ok_or("T08A source path")?;
        ensure(
            !Path::new(path).is_absolute() && !path.split('/').any(|p| p == ".."),
            "T08A source path escape",
        )?;
        let full = root.join(path);
        let metadata = fs::symlink_metadata(&full).map_err(|e| e.to_string())?;
        ensure(
            metadata.file_type().is_file()
                && metadata.len() == row["bytes"].as_u64().ok_or("source byte count")?
                && full
                    .canonicalize()
                    .map_err(|e| e.to_string())?
                    .starts_with(root.canonicalize().map_err(|e| e.to_string())?),
            "T08A regular exact-size source within root",
        )?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            ensure(
                row["unix_modes"].as_array().is_some_and(|allowed| {
                    allowed.iter().any(|mode| {
                        mode.as_u64() == Some(u64::from(metadata.permissions().mode() & 0o7777))
                    })
                }),
                "T08A physical Unix source mode",
            )?;
        }
        #[cfg(windows)]
        {
            let expected = if row["mode"].as_u64() == Some(0o755) {
                "100755 "
            } else {
                "100644 "
            };
            let mode = command(
                root,
                "git",
                &["ls-files", "--stage", "--", path],
                &mut vec![],
            )?;
            ensure(
                mode.starts_with(expected)
                    && mode.lines().count() == 1
                    && mode.ends_with(&format!("\t{path}\n")),
                "T08A Windows source index mode",
            )?;
        }
        let bytes = fs::read(full).map_err(|e| e.to_string())?;
        ensure(
            hash(&bytes) == row["sha256"],
            &format!("T08A source changed: {path}"),
        )?;
    }
    Ok(embedded)
}
// Windows completed250 consecutive cases in298s before the old300s suite cap.
// Full261+4 suites get a fixed900s ceiling; metadata/help retain300s. Each
// child still has its25s contract, and the whole hosted job remains60minutes.
#[derive(Clone, Copy)]
enum NativeChildScope {
    Metadata,
    FullCorpus,
}
impl NativeChildScope {
    const fn deadline(self) -> Duration {
        Duration::from_secs(match self {
            Self::Metadata => 300,
            Self::FullCorpus => 900,
        })
    }
    const fn name(self) -> &'static str {
        match self {
            Self::Metadata => "metadata",
            Self::FullCorpus => "full-corpus",
        }
    }
}
fn bounded(
    root: &Path,
    image: &Path,
    args: &[&str],
    scope: NativeChildScope,
    env: &[(&str, &Path)],
    records: &mut Vec<Value>,
) -> Result<String> {
    let mut c = Command::new(image);
    c.args(args)
        .current_dir(root)
        .env_clear()
        .env("PATH", "/nonexistent");
    for (k, v) in env {
        c.env(k, v);
    }
    t08a_child::windows_system_environment(&mut c);
    let captured = t08a_child::capture(c, scope.deadline(), 8 * 1024 * 1024, 128 * 1024)?;
    records.push(json!({"program":image,"argv":args,"capture_scope":scope.name(),"deadline_seconds":scope.deadline().as_secs(),"pid":captured.pid,"status":captured.status,"stdout":String::from_utf8_lossy(&captured.stdout),"stderr":String::from_utf8_lossy(&captured.stderr),"error":captured.error,"cleanup":captured.cleanup,"image_sha256":hash(&fs::read(image).map_err(|e|e.to_string())?)}));
    ensure(
        captured.error.is_none() && captured.status == Some(0),
        "T08A native child/output/cleanup failed",
    )?;
    String::from_utf8(captured.stdout).map_err(|e| e.to_string())
}

fn runtime_audit(root: &Path, image: &Path, records: &mut Vec<Value>) -> Result<()> {
    let dependencies = if cfg!(target_os = "macos") {
        command(
            root,
            "/usr/bin/otool",
            &["-L", image.to_str().ok_or("image UTF8")?],
            records,
        )?
    } else if cfg!(target_os = "linux") {
        command(
            root,
            "/usr/bin/ldd",
            &[image.to_str().ok_or("image UTF8")?],
            records,
        )?
    } else if cfg!(windows) {
        let base = std::env::var_os("ProgramFiles(x86)")
            .map(PathBuf::from)
            .ok_or("Windows installer root")?;
        let vswhere = base.join("Microsoft Visual Studio/Installer/vswhere.exe");
        let locations = command(
            root,
            vswhere.to_str().ok_or("vswhere UTF8")?,
            &[
                "-latest",
                "-products",
                "*",
                "-find",
                r"VC\Tools\MSVC\**\bin\Hostx64\x64\dumpbin.exe",
            ],
            records,
        )?;
        let tool = locations.lines().next().ok_or("MSVC dumpbin unavailable")?;
        records.push(json!({"runtime_audit_tool":tool,"sha256":hash(&fs::read(tool).map_err(|e|e.to_string())?)}));
        command(
            root,
            tool,
            &["/DEPENDENTS", image.to_str().ok_or("image UTF8")?],
            records,
        )?
    } else {
        return Err("T08A runtime audit unsupported platform".into());
    };
    let dependencies = dependencies.to_ascii_lowercase();
    for forbidden in [
        "libpython",
        "python3.dll",
        "python3.",
        "pypy",
        "libssl.",
        "libcrypto.",
        "libssl-",
        "libcrypto-",
    ] {
        ensure(
            !dependencies.contains(forbidden),
            "T08A interpreter/dynamic OpenSSL runtime dependency",
        )?;
    }
    Ok(())
}
pub(crate) fn native_host(root: &Path, records: &mut Vec<Value>) -> Result<String> {
    let raw = command(root, "rustc", &["--version", "--verbose"], records)?;
    let hosts = raw
        .lines()
        .filter_map(|line| line.strip_prefix("host: "))
        .collect::<Vec<_>>();
    ensure(
        hosts.len() == 1
            && matches!(
                hosts[0],
                "x86_64-unknown-linux-gnu"
                    | "aarch64-unknown-linux-gnu"
                    | "x86_64-apple-darwin"
                    | "aarch64-apple-darwin"
                    | "x86_64-pc-windows-msvc"
            ),
        "T08A supported actual native host required",
    )?;
    Ok(hosts[0].to_owned())
}
fn validate_shipping_context(root: &Path, rows: &[Value]) -> Result<Value> {
    ensure(
        rows.last()
            .is_some_and(|r| r["reason"] == "build-finished" && r["success"] == true),
        "T08A default compiler completion",
    )?;
    let cli_manifest = root
        .join("crates/ssx-cli/Cargo.toml")
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let private_manifest = root
        .join("vendor/ssx-cli-hyper/Cargo.toml")
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let cli = rows
        .iter()
        .filter(|r| {
            r["reason"] == "compiler-artifact"
                && r["manifest_path"]
                    .as_str()
                    .and_then(|p| Path::new(p).canonicalize().ok())
                    .as_ref()
                    == Some(&cli_manifest)
                && r["target"]["name"] == "ssx"
                && r["executable"].is_string()
        })
        .collect::<Vec<_>>();
    ensure(cli.len() == 1, "T08A exact default CLI context")?;
    ensure(
        cli[0]["profile"]["test"] == false,
        "T08A test-profile CLI cannot be default shipping",
    )?;
    ensure(
        cli[0]["features"].as_array().is_some_and(Vec::is_empty),
        "T08A non-default CLI features cannot be default shipping",
    )?;
    let private = rows
        .iter()
        .filter(|r| {
            r["reason"] == "compiler-artifact"
                && r["manifest_path"]
                    .as_str()
                    .and_then(|p| Path::new(p).canonicalize().ok())
                    .as_ref()
                    == Some(&private_manifest)
        })
        .collect::<Vec<_>>();
    ensure(
        private.len() == 1,
        "T08A exact default private dependency context",
    )?;
    ensure(
        private[0]["profile"]["test"] == false,
        "T08A test-profile private dependency cannot be default shipping",
    )?;
    let features = private[0]["features"]
        .as_array()
        .ok_or("private default feature inventory")?
        .iter()
        .map(|f| f.as_str().ok_or("private feature name"))
        .collect::<std::result::Result<BTreeSet<_>, _>>()?;
    ensure(
        features == BTreeSet::from(["client", "http1", "source-client-framing"]),
        "T08A qualification features leaked into default CLI",
    )?;
    let image = PathBuf::from(cli[0]["executable"].as_str().ok_or("default image")?);
    let metadata = fs::symlink_metadata(&image).map_err(|e| e.to_string())?;
    ensure(
        metadata.file_type().is_file(),
        "T08A regular default compiler image",
    )?;
    Ok(
        json!({"role":"standalone-default-shipping-floor","CLI":cli[0],"private_dependency":private[0],"image_path":image,"image_sha256":hash(&fs::read(&image).map_err(|e|e.to_string())?),"bytes":metadata.len(),"scope":"actual standalone default compiler context; not external-test or qualification-full image context"}),
    )
}
fn default_shipping_floor(root: &Path, records: &mut Vec<Value>) -> Result<(PathBuf, String)> {
    let raw = command(
        root,
        "cargo",
        &[
            "build",
            "--locked",
            "-p",
            "ssx-cli",
            "--bin",
            "ssx",
            "--message-format=json",
        ],
        records,
    )?;
    let rows = raw
        .lines()
        .map(serde_json::from_str::<Value>)
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    // The real successful default transcript is validated and recorded FIRST.
    let proof = validate_shipping_context(root, &rows)?;
    let image = PathBuf::from(proof["image_path"].as_str().ok_or("default image path")?);
    let image_hash = proof["image_sha256"]
        .as_str()
        .ok_or("default image hash")?
        .to_owned();
    let parent_hash = hash(&serde_json::to_vec(&rows).map_err(|e| e.to_string())?);
    records.push(proof.clone());
    for (selector, pointer, replacement, expected) in [
        (
            "private_dependency",
            "/features",
            json!([
                "client",
                "default",
                "full",
                "http1",
                "http2",
                "server",
                "source-client-framing"
            ]),
            "T08A qualification features leaked into default CLI",
        ),
        (
            "CLI",
            "/features",
            json!(["source-transport-test-fixtures"]),
            "T08A non-default CLI features cannot be default shipping",
        ),
        (
            "CLI",
            "/profile/test",
            json!(true),
            "T08A test-profile CLI cannot be default shipping",
        ),
        (
            "private_dependency",
            "/profile/test",
            json!(true),
            "T08A test-profile private dependency cannot be default shipping",
        ),
    ] {
        let index = rows
            .iter()
            .position(|row| row == &proof[selector])
            .ok_or("actual default context row")?;
        let mut mutant = rows.clone();
        let slot = mutant[index]
            .pointer_mut(pointer)
            .ok_or("actual default context field")?;
        let original = slot.clone();
        *slot = replacement.clone();
        // All other actual producer fields, including profile/features/manifest
        // identities not selected above, remain byte-equivalent JSON values.
        let mut restored = mutant.clone();
        *restored[index]
            .pointer_mut(pointer)
            .ok_or("restore context field")? = original.clone();
        ensure(
            restored == rows,
            "T08A context control changed unrelated field",
        )?;
        let rejected = validate_shipping_context(root, &mutant).err();
        ensure(
            rejected.as_deref() == Some(expected),
            "T08A context control did not reach named rejection",
        )?;
        records.push(json!({"role":"default-shipping-context-negative-control","actual_cargo_stdout_sha256":hash(raw.as_bytes()),"parent_parsed_context_sha256":parent_hash,"mutant_parsed_context_sha256":hash(&serde_json::to_vec(&mutant).map_err(|e|e.to_string())?),"changed_json_pointer":format!("/{index}{pointer}"),"row_package_id":proof[selector]["package_id"],"original":original,"replacement":replacement,"expected_rejection":expected,"actual_rejection":rejected,"scope":"actual producer-transcript metadata mutation only; not runtime behavior/platform acceptance"}));
    }
    Ok((image, image_hash))
}
pub fn check(root: &Path, records: &mut Vec<Value>) -> Result<()> {
    let before = sources(root)?;
    vendor_sources(root)?;
    process_wrap_guard::sources(root)?;
    source_custody::controls(root)?;
    tls_roles::check(root, records)?;
    t08a_child::controls(
        &std::env::current_exe().map_err(|e| e.to_string())?,
        records,
    )?;
    ensure(
        hash(&fs::read(root.join("ssx")).map_err(|e| e.to_string())?)
            == "4c45e876fc79dffc0f7da8ccccc9c6ba67d4f4cc9e0bd9e8194f74184b96c65b",
        "T08A historical optional wire-client drift",
    )?;
    let corpus: Value = serde_json::from_slice(
        &fs::read(root.join("docs/rust-rewrite/t08a/cases.json")).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let inventory: Vec<String> = serde_json::from_slice(
        &fs::read(root.join("docs/rust-rewrite/t08a/inventory.json")).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let ids = corpus["cases"]
        .as_array()
        .ok_or("T08A cases missing")?
        .iter()
        .map(|c| c["id"].as_str().ok_or_else(|| "T08A case id".to_owned()))
        .collect::<Result<Vec<_>>>()?;
    ensure(
        ids == inventory.iter().map(String::as_str).collect::<Vec<_>>()
            && ids.iter().collect::<BTreeSet<_>>().len() == ids.len(),
        "T08A missing/duplicate/reordered case",
    )?;
    let listing = command(
        root,
        "cargo",
        &[
            "test",
            "--locked",
            "-p",
            "ssx-cli-openssl-store",
            "--",
            "--list",
            "--format",
            "terse",
        ],
        records,
    )?;
    let mut names = listing
        .lines()
        .filter_map(|l| l.strip_suffix(": test"))
        .collect::<Vec<_>>();
    names.sort_unstable();
    let mut expected = vec![
        "tests::copied_uri_outlives_input_and_lazily_loads_matching_dot_one",
        "tests::interior_nul_is_rejected_without_mutating_or_freeing_store",
        "tests::unavailable_uri_returns_native_error_without_owning_store",
    ];
    #[cfg(unix)]
    expected.push("tests::owned_os_file_and_hash_directory_preserve_bytes_and_validation");
    expected.sort_unstable();
    ensure(
        names == expected,
        "T08A exact FFI control inventory differs",
    )?;
    let unit = command(
        root,
        "cargo",
        &[
            "test",
            "--locked",
            "-p",
            "ssx-cli-openssl-store",
            "--",
            "--test-threads=1",
        ],
        records,
    )?;
    ensure(
        unit.contains(if cfg!(unix) {
            "4 passed; 0 failed"
        } else {
            "3 passed; 0 failed"
        }),
        "T08A OpenSSL store FFI controls missing",
    )?;
    let host = native_host(root, records)?;
    let metadata = command(
        root,
        "cargo",
        &[
            "metadata",
            "--locked",
            "--no-deps",
            "--format-version",
            "1",
            "--filter-platform",
            &host,
        ],
        records,
    )?;
    let metadata: Value = serde_json::from_str(&metadata).map_err(|e| e.to_string())?;
    let target = PathBuf::from(
        metadata["target_directory"]
            .as_str()
            .ok_or("target directory")?,
    );
    process_wrap_guard::qualify(root, &host, &target, records)?;
    let output = target.join("rust-evidence/t08a-native");
    fs::create_dir_all(&output).map_err(|e| e.to_string())?;
    let tree = command(
        root,
        "cargo",
        &[
            "tree", "--locked", "-p", "ssx-cli", "--edges", "normal", "--prefix", "none",
        ],
        records,
    )?;
    for line in tree.lines() {
        let name = line.split_whitespace().next().unwrap_or("");
        ensure(
            !name.starts_with("ssx-")
                || matches!(
                    name,
                    "ssx-cli"
                        | "ssx-cli-openssl-store"
                        | "ssx-cli-hyper"
                        | "ssx-json"
                        | "ssx-source-codecs"
                        | "ssx-owned-budget"
                ),
            "T08A unexpected backend/shared-service crate",
        )?;
    }
    for forbidden in [
        "ssx-http ",
        "ssx-mcp ",
        "ssx-infra ",
        "ssx-application ",
        "ssx-domain ",
        "ssx-wire ",
        "process-wrap ",
        "pyo3 ",
        "cpython ",
        "python ",
    ] {
        ensure(
            !tree.contains(forbidden),
            "T08A backend/interpreter runtime dependency",
        )?;
    }
    let units = command(
        root,
        "cargo",
        &[
            "test", "--locked", "-p", "ssx-cli", "--bin", "ssx", "--", "--list", "--format",
            "terse",
        ],
        records,
    )?;
    let actual = units
        .lines()
        .filter_map(|line| line.strip_suffix(": test"))
        .collect::<BTreeSet<_>>();
    let expected = include_str!("../../../docs/rust-rewrite/t08a/cli-unit-controls.txt")
        .lines()
        .chain(if cfg!(target_os = "macos") { Some("proxy::tests::native_cf_proxy_exception_strings_roundtrip_without_raw_pointer_casts") } else { None })
        .collect::<BTreeSet<_>>();
    ensure(
        actual == expected,
        "T08A exact CLI mechanism controls differ",
    )?;
    command(
        root,
        "cargo",
        &[
            "test",
            "--locked",
            "-p",
            "ssx-cli",
            "--bin",
            "ssx",
            "--",
            "--test-threads=1",
        ],
        records,
    )?;
    command(
        root,
        "cargo",
        &[
            "test",
            "--locked",
            "-p",
            "ssx-source-codecs",
            "--lib",
            "--",
            "--test-threads=1",
        ],
        records,
    )?;
    command(root, "cargo", PRIVATE_LIB_LINT_ARGS, records)?;
    let private_listing = command(
        root,
        "cargo",
        &[
            "test",
            "--locked",
            "-p",
            "ssx-cli-hyper",
            "--features",
            "full,source-client-framing",
            "--lib",
            "--",
            "--list",
            "--format",
            "terse",
        ],
        records,
    )?;
    let actual = private_listing
        .lines()
        .filter_map(|line| line.strip_suffix(": test"))
        .filter(|name| name.contains("source_raw_target") || name.contains("source_framing"))
        .collect::<BTreeSet<_>>();
    let expected = include_str!("../../../docs/rust-rewrite/t08a/private-http-controls.txt")
        .lines()
        .collect::<BTreeSet<_>>();
    ensure(
        actual == expected,
        "T08A exact private HTTP controls differ",
    )?;
    command(
        root,
        "cargo",
        &[
            "test",
            "--locked",
            "-p",
            "ssx-cli-hyper",
            "--features",
            "full,source-client-framing",
            "--lib",
            "--",
            "--test-threads=1",
        ],
        records,
    )?;
    private_qualification_controls(records)?;
    let built = command(
        root,
        "cargo",
        &[
            "test",
            "--locked",
            "-p",
            "ssx-cli",
            "--test",
            "external",
            "--no-run",
            "--message-format=json",
        ],
        records,
    )?;
    let mut image = None;
    let mut test = None;
    let mut compiler = Vec::new();
    for line in built.lines() {
        let Ok(v) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if v["reason"] != "compiler-artifact" {
            continue;
        }
        if let Some(exe) = v["executable"].as_str() {
            let p = PathBuf::from(exe);
            let bytes = fs::read(&p).map_err(|e| e.to_string())?;
            compiler.push(json!({"role":"external-test-compiler-snapshot-before-default-floor","artifact":v,"sha256":hash(&bytes),"bytes":bytes.len(),"scope":"pre-floor compiler context; same target path may later contain different default-role bytes"}));
            match v["target"]["name"].as_str() {
                Some("ssx") => image = Some(p),
                Some("external") => test = Some(p),
                _ => {}
            }
        }
    }
    let external_build_image = image.ok_or("actual compiler CLI image absent")?;
    let test = test.ok_or("actual compiler external test image absent")?;
    let (image, shipping_hash) = default_shipping_floor(root, records)?;
    ensure(
        image == external_build_image,
        "T08A default/external compiler target path differs",
    )?;
    runtime_audit(root, &image, records)?;
    let shipping_image = json!({"role":"actual-standalone-default-image-used-for-external-run","path":&image,"sha256":&shipping_hash,"CLI_features":[],"private_features":["client","http1","source-client-framing"],"scope":"independent of pre-floor external compiler snapshots and separate from installed image"});
    fs::write(
        output.join("default-shipping-image.json"),
        serde_json::to_vec_pretty(&shipping_image).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let tmp = std::env::var_os("TMPDIR")
        .map(PathBuf::from)
        .ok_or("T08A requires registered TMPDIR")?;
    let env = [
        ("SSX_T08A_BIN", image.as_path()),
        ("SSX_T08A_ROOT", root),
        ("SSX_T08A_EVIDENCE", output.as_path()),
        ("TMPDIR", tmp.as_path()),
    ];
    let listing = bounded(
        root,
        &test,
        &["--list", "--format", "terse"],
        NativeChildScope::Metadata,
        &env,
        records,
    )?;
    ensure(
        listing
            .lines()
            .filter_map(|l| l.strip_suffix(": test"))
            .collect::<BTreeSet<_>>()
            == BTreeSet::from([
                "missing_case_and_mutated_native_reply_are_rejected",
                "native_external_child_executes_complete_frozen_contract",
            ]),
        "T08A executable test inventory differs",
    )?;
    bounded(
        root,
        &test,
        &["--test-threads=1", "--nocapture"],
        NativeChildScope::FullCorpus,
        &env,
        records,
    )?;
    ensure(
        hash(&fs::read(&image).map_err(|e| e.to_string())?) == shipping_hash,
        "T08A standalone default image changed through external run",
    )?;
    // Installation reuses the same debug profile/target; no duplicate Cargo cache.
    let install = output.join("install");
    let install_s = install.to_str().ok_or("install UTF8")?;
    command(
        root,
        "cargo",
        &[
            "install",
            "--locked",
            "--debug",
            "--path",
            "crates/ssx-cli",
            "--root",
            install_s,
            "--force",
        ],
        records,
    )?;
    let installed = install
        .join("bin")
        .join(if cfg!(windows) { "ssx.exe" } else { "ssx" });
    bounded(
        root,
        &installed,
        &["--help"],
        NativeChildScope::Metadata,
        &[],
        records,
    )?;
    runtime_audit(root, &installed, records)?;
    let installed_evidence = output.join("installed");
    let installed_env = [
        ("SSX_T08A_BIN", installed.as_path()),
        ("SSX_T08A_ROOT", root),
        ("SSX_T08A_EVIDENCE", installed_evidence.as_path()),
        ("TMPDIR", tmp.as_path()),
    ];
    bounded(
        root,
        &test,
        &[
            "native_external_child_executes_complete_frozen_contract",
            "--exact",
            "--test-threads=1",
        ],
        NativeChildScope::FullCorpus,
        &installed_env,
        records,
    )?;
    ensure(
        sources(root)? == before,
        "T08A source changed during execution",
    )?;
    fs::write(
        output.join("compiler-images.json"),
        serde_json::to_vec_pretty(&compiler).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    records.push(json!({"plan":"T08A","finite_cases":ids,"source_bindings":before,"compiler_images_role":"external-build snapshots before default-floor build; path equality is not byte identity","compiler_images":compiler,"standalone_default_image":shipping_image,"installed_image":{"sha256":hash(&fs::read(installed).map_err(|e|e.to_string())?)},"scope":"real standalone external-child loopback and source install, not native backend/full rewrite acceptance"}));
    Ok(())
}

fn vendor_sources(root: &Path) -> Result<()> {
    let provenance: Value = serde_json::from_slice(
        &fs::read(root.join("docs/rust-rewrite/t08a/hyper-provenance.json"))
            .map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let lock = fs::read_to_string(root.join("Cargo.lock")).map_err(|e| e.to_string())?;
    let version = provenance["upstream"]["version"]
        .as_str()
        .ok_or("upstream version")?;
    let checksum = provenance["upstream"]["archive_sha256"]
        .as_str()
        .ok_or("upstream checksum")?;
    ensure(
        lock.split("[[package]]").any(|p| {
            p.lines().any(|l| l == "name = \"hyper\"")
                && p.lines().any(|l| l == format!("version = \"{version}\""))
                && p.lines().any(|l| l == format!("checksum = \"{checksum}\""))
        }),
        "private Hyper upstream registry/advisory identity changed without reviewed port",
    )?;
    let archive = fs::read(root.join("docs/rust-rewrite/t08a/hyper-1.12.0.crate"))
        .map_err(|e| e.to_string())?;
    ensure(
        hash(&archive) == checksum,
        "private Hyper pristine archive changed",
    )?;
    let vendor = root.join("vendor/ssx-cli-hyper");
    let mut actual = BTreeSet::new();
    let mut pending = vec![vendor.clone()];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(directory).map_err(|e| e.to_string())? {
            let path = entry.map_err(|e| e.to_string())?.path();
            let metadata = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
            ensure(!metadata.file_type().is_symlink(), "private Hyper symlink")?;
            if metadata.is_dir() {
                pending.push(path)
            } else {
                ensure(metadata.is_file(), "private Hyper non-file")?;
                actual.insert(
                    path.strip_prefix(&vendor)
                        .map_err(|e| e.to_string())?
                        .to_string_lossy()
                        .replace('\\', "/"),
                );
            }
        }
    }
    let mut expected = BTreeSet::new();
    for row in provenance["files"]
        .as_array()
        .ok_or("private Hyper finite files")?
    {
        let path = row["path"].as_str().ok_or("private Hyper path")?;
        ensure(
            !Path::new(path).is_absolute() && !path.split('/').any(|c| c == ".."),
            "private Hyper path escape",
        )?;
        expected.insert(path.to_owned());
        ensure(
            hash(&fs::read(vendor.join(path)).map_err(|e| e.to_string())?) == row["sha256"],
            "private Hyper file changed",
        )?;
    }
    ensure(actual == expected, "private Hyper missing or extra files")
}
