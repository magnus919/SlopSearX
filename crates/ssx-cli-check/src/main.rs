//! Backend-free developer runner for the exact shared T08A gate. Never a runtime sidecar.
#[path = "../../xtask/src/t08a.rs"]
mod t08a;
#[path = "../../xtask/src/t08a_child.rs"]
mod t08a_child;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::Command,
};
type Result<T> = std::result::Result<T, String>;
fn command(root: &Path, program: &str, args: &[&str], records: &mut Vec<Value>) -> Result<String> {
    let output = Command::new(program)
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|e| format!("{program}: {e}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    records.push(json!({"program":program,"args":args,"exit_code":output.status.code(),"stdout":stdout,"stderr":String::from_utf8_lossy(&output.stderr)}));
    if !output.status.success() {
        return Err(format!("command failed: {program} {}", args.join(" ")));
    }
    Ok(stdout)
}
fn load_json(path: &Path) -> Result<Value> {
    serde_json::from_slice(&fs::read(path).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}
fn hash_bytes(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn digest(path: &Path) -> Result<String> {
    Ok(Sha256::digest(fs::read(path).map_err(|e| e.to_string())?)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}
// F02 accepts this exact stable release; environment overrides must not bypass it.
const ACCEPTED_RUST: &str = "1.99.0";
fn validate_toolchain(pins: &Value, configuration: &str, actual: &str) -> Result<()> {
    if pins["toolchain"]["rust"].as_str() != Some(ACCEPTED_RUST) {
        return Err("Rust architecture pin differs from accepted F02 release".into());
    }
    let mut in_toolchain = false;
    let mut channels = Vec::new();
    for line in configuration.lines() {
        let line = line.split('#').next().unwrap_or_default().trim();
        if line.starts_with('[') {
            in_toolchain = line == "[toolchain]";
        } else if in_toolchain
            && let Some((key, value)) = line.split_once('=')
            && key.trim() == "channel"
        {
            channels.push(value.trim());
        }
    }
    if channels != [format!("\"{ACCEPTED_RUST}\"")] {
        return Err("rust-toolchain.toml must select the accepted F02 Rust release".into());
    }
    let mut lines = actual.lines();
    let header = lines.next().unwrap_or_default();
    let header_release = header
        .strip_prefix("rustc ")
        .and_then(|s| s.split_once(" ("));
    let releases: Vec<_> = lines
        .filter_map(|line| line.strip_prefix("release: "))
        .collect();
    if !header_release.is_some_and(|(release, build)| {
        release == ACCEPTED_RUST && build.ends_with(')') && build.len() > 1
    }) || releases != [ACCEPTED_RUST]
    {
        return Err(format!(
            "actual rustc identity must report accepted F02 release {ACCEPTED_RUST}"
        ));
    }
    Ok(())
}
fn check_toolchain(root: &Path, actual: &str) -> Result<()> {
    let pins = load_json(&root.join("docs/rust-rewrite/architecture/pins.json"))?;
    let configuration = fs::read_to_string(root.join("rust-toolchain.toml"))
        .map_err(|e| format!("Rust toolchain configuration: {e}"))?;
    validate_toolchain(&pins, &configuration, actual)
}
fn resolve_output(path: &Path) -> Result<PathBuf> {
    if fs::symlink_metadata(path).is_ok() {
        return path
            .canonicalize()
            .map_err(|e| format!("output destination: {e}"));
    }
    let parent = path.parent().ok_or("output has no parent")?;
    Ok(resolve_output(parent)?.join(path.file_name().ok_or("output has no filename")?))
}
fn validate_output(root: &Path, path: &Path) -> Result<PathBuf> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|e| e.to_string())?
            .join(path)
    };
    let destination = resolve_output(&absolute)?;
    for option in ["--git-dir", "--git-common-dir"] {
        let directory = command(
            root,
            "git",
            &["rev-parse", "--path-format=absolute", option],
            &mut vec![],
        )?;
        let directory = Path::new(directory.trim())
            .canonicalize()
            .map_err(|e| format!("repository metadata directory: {e}"))?;
        if destination.starts_with(directory) {
            return Err("evidence output overlaps repository metadata".into());
        }
    }
    if let Ok(relative) = destination.strip_prefix(root) {
        let name = relative.to_str().ok_or("non-UTF8 output path")?;
        let owned = name == ".git"
            || name.starts_with(".git/")
            || name.starts_with("contracts/")
            || name.starts_with("crates/")
            || name.starts_with(".cargo/")
            || name.starts_with("docs/rust-rewrite/")
            || name.starts_with("docs/adr/")
            || matches!(
                name,
                "Cargo.toml"
                    | "Cargo.lock"
                    | "rust-toolchain.toml"
                    | "deny.toml"
                    | ".github/workflows/rust.yml"
                    | "scripts/rust-foundation-smoke.sh"
            );
        let tracked = command(root, "git", &["ls-files", "-z"], &mut vec![])?;
        let tracked = tracked.split('\0').any(|p| p == name);
        let ignored = Command::new("git")
            .args(["check-ignore", "--quiet", "--", name])
            .current_dir(root)
            .status()
            .map_err(|e| e.to_string())?
            .success();
        if owned || tracked || !ignored {
            return Err("evidence output overlaps repository source or non-ignored files".into());
        }
    }
    Ok(destination)
}
fn write_evidence(root: &Path, path: &Path, evidence: &Value) -> Result<()> {
    let destination = validate_output(root, path)?;
    fs::create_dir_all(destination.parent().ok_or("output has no parent")?)
        .map_err(|e| e.to_string())?;
    let destination = validate_output(root, path)?;
    // Atomic replacement avoids modifying a hardlinked source inode.
    let temporary = destination.with_file_name(format!(
        ".ssx-evidence-{}-{}.tmp",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_nanos()
    ));
    let result = (|| -> Result<()> {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|e| e.to_string())?;
        file.write_all(&serde_json::to_vec_pretty(evidence).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        fs::rename(&temporary, &destination).map_err(|e| e.to_string())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn source_snapshot(root: &Path, records: &mut Vec<Value>) -> Result<Value> {
    let names = command(
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
    let mut files = Vec::new();
    for name in names.split('\0').filter(|p| !p.is_empty()) {
        let path = root.join(name);
        let metadata = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
        if !metadata.file_type().is_file()
            || !path
                .canonicalize()
                .map_err(|e| e.to_string())?
                .starts_with(root)
        {
            return Err("non-regular/outside source input".into());
        }
        files.push(json!({"path":name,"bytes":metadata.len(),"sha256":digest(&path)?}));
    }
    files.sort_by(|a, b| a["path"].as_str().cmp(&b["path"].as_str()));
    if files.is_empty() {
        return Err("empty source snapshot".into());
    }
    Ok(
        json!({"head":command(root,"git",&["rev-parse","HEAD"],records)?.trim(),"working_tree":command(root,"git",&["status","--porcelain=v1","--untracked-files=all"],records)?,"index_sha256":hash_bytes(command(root,"git",&["ls-files","--stage","-z"],&mut vec![])?.as_bytes()),"files":files}),
    )
}
fn execute(root: &Path, args: &[String], records: &mut Vec<Value>, before: &Value) -> Result<()> {
    if !((args.len() == 2 || args.len() == 4 && args[2] == "--output")
        && args[0] == "check"
        && matches!(args[1].as_str(), "T08A" | "T08A-TLS-ROLES"))
    {
        return Err("usage: ssx-cli-check check T08A|T08A-TLS-ROLES [--output PATH]".into());
    }
    if args.len() == 4 {
        validate_output(root, Path::new(&args[3]))?;
    }
    let graph = command(
        root,
        "cargo",
        &[
            "tree",
            "--locked",
            "-p",
            "ssx-cli-check",
            "--edges",
            "normal",
            "--prefix",
            "none",
        ],
        records,
    )?;
    for line in graph.lines() {
        let name = line.split_whitespace().next().unwrap_or("");
        if name.starts_with("ssx-") && name != "ssx-cli-check"
            || ["libxml", "pyo3", "cpython", "python"]
                .iter()
                .any(|p| name.contains(p))
        {
            return Err("backend/interpreter dependency in portable checker".into());
        }
    }
    match args[1].as_str() {
        "T08A-TLS-ROLES" => t08a::tls_roles::check(root, records)?,
        _ => t08a::check(root, records)?,
    }
    let after = source_snapshot(root, &mut vec![])?;
    if *before != after {
        return Err("source changed during T08A gate".into());
    }
    Ok(())
}
fn main() {
    if let Some(code) = t08a_child::fixture_entry() {
        std::process::exit(code);
    }
    let root = std::env::current_dir()
        .ok()
        .and_then(|p| {
            p.ancestors()
                .find(|p| {
                    p.join("docs/rust-rewrite/t08a/cases.json").is_file()
                        && p.join("Cargo.toml").is_file()
                })
                .map(Path::to_owned)
        })
        .and_then(|p| p.canonicalize().ok())
        .expect("run within the current workspace");
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let mut records = Vec::new();
    let mut identity = Vec::new();
    let image = std::env::current_exe().expect("current checker image");
    let image_before = digest(&image);
    let started = std::time::SystemTime::now();
    let source = source_snapshot(&root, &mut identity);
    let toolchain = command(&root, "rustc", &["--version", "--verbose"], &mut identity);
    let mut result = match (&source, &toolchain) {
        (Ok(source), Ok(toolchain)) => check_toolchain(&root, toolchain)
            .and_then(|_| execute(&root, &args, &mut records, source)),
        (Err(error), _) | (_, Err(error)) => Err(error.clone()),
    };
    if result.is_ok() && image_before.as_ref().ok() != digest(&image).as_ref().ok() {
        result = Err("checker image changed during execution".into());
    }
    let plan_id = if args.get(1).map(String::as_str) == Some("T08A-TLS-ROLES") {
        "T08A-TLS-ROLES"
    } else {
        "T08A"
    };
    let scope = if plan_id == "T08A-TLS-ROLES" {
        "locked dependency/provenance policy only; no CLI runtime/backend/platform acceptance"
    } else {
        "standalone external-HTTP CLI and source install; no native backend/full rewrite acceptance"
    };
    let mut evidence = json!({"schema_version":1,"plan_id":plan_id,"runner":"backend-free ssx-cli-check, exact shared crates/xtask/src/t08a.rs","status":if result.is_ok(){"passed"}else{"failed"},"source":source.as_ref().ok(),"toolchain":toolchain.as_ref().ok(),"checker_image":{"path":image,"sha256":image_before.ok()},"started_unix_ms":started.duration_since(std::time::UNIX_EPOCH).expect("wall clock").as_millis(),"finished_unix_ms":std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).expect("wall clock").as_millis(),"identity_commands":identity,"commands":records,"error":result.err(),"scope":scope});
    if args.len() == 4
        && args[2] == "--output"
        && let Err(error) = write_evidence(&root, Path::new(&args[3]), &evidence)
    {
        evidence["status"] = json!("failed");
        evidence["error"] = json!(error);
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&evidence).expect("evidence JSON")
    );
    if evidence["status"] != "passed" {
        std::process::exit(1)
    }
}
