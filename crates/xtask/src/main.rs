//! Fail-closed native evidence runner. Owned check implementations are deliberately finite.
mod a01;
mod a02;
mod a03;
mod a04;
mod a05;
mod a06;
mod a06_resource;
mod a07;
mod a08;
mod a09;
mod a10;
mod a11;
mod a12;
mod a13;
mod a14;
mod date_amendment;
mod f05;
mod f06;
mod f07;
mod f08;
mod f09;
mod f10;
mod f11;
mod f12;
mod f13;
mod f14;
mod f15;
mod o01;
mod o03;
mod o06;
mod replay;
#[path = "../../../docs/rust-rewrite/snapshot-fixture-amendment/validator.rs"]
mod snapshot_fixture_amendment;
mod t01;
mod t02;
mod t04;
mod t05;
mod t06;
mod t08a;
mod t08a_child;
mod t09;
mod w00;
mod w01;
mod w02;
mod w03;
mod w04;
mod w05;
mod w06;
mod w06_source_amendment;
mod w07;
mod w11;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    fs,
    io::{BufRead, BufReader, Read, Write},
    net::TcpStream,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::Duration,
};
type Result<T> = std::result::Result<T, String>;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Fixture {
    schema_version: u32,
    plan_id: String,
    sources: Vec<Source>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Source {
    path: String,
    git_blob: String,
}
fn command(root: &Path, program: &str, args: &[&str], records: &mut Vec<Value>) -> Result<String> {
    let output = Command::new(program)
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|e| format!("{program}: {e}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    records.push(json!({"program": program, "args": args, "exit_code": output.status.code(), "stdout": stdout, "stderr": String::from_utf8_lossy(&output.stderr)}));
    if !output.status.success() {
        return Err(format!("command failed: {program} {}", args.join(" ")));
    }
    Ok(stdout)
}
fn load_json(path: &Path) -> Result<Value> {
    serde_json::from_slice(&fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?)
        .map_err(|e| format!("{}: {e}", path.display()))
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
fn validate_fixture(f: &Fixture, id: &str) -> Result<()> {
    if f.schema_version != 1 || f.plan_id != id || f.sources.is_empty() {
        return Err("invalid or empty fixture".into());
    }
    let mut seen = BTreeSet::new();
    for s in &f.sources {
        if s.path.is_empty()
            || Path::new(&s.path).is_absolute()
            || s.path.split('/').any(|p| p == "..")
            || !seen.insert(&s.path)
            || s.git_blob.len() != 40
            || !s.git_blob.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err("invalid/duplicate source fixture".into());
        }
    }
    Ok(())
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
fn source_snapshot(root: &Path, records: &mut Vec<Value>) -> Result<Vec<Value>> {
    let paths = command(
        root,
        "git",
        &[
            "ls-files",
            "--cached",
            "--others",
            "--exclude-standard",
            "-z",
        ],
        records,
    )?;
    let mut sources = Vec::new();
    for path in paths.split('\0').filter(|p| !p.is_empty()).filter(|p| {
        p.starts_with("contracts/")
            || p.starts_with("crates/")
            || p.starts_with(".cargo/")
            || p.starts_with(".github/workflows/")
            || matches!(
                *p,
                ".github/pr-review-publisher-policy.json"
                    | ".github/PULL_REQUEST_TEMPLATE.md"
                    | ".pre-commit-config.yaml"
            )
            || matches!(
                *p,
                "Cargo.toml"
                    | "Cargo.lock"
                    | "rust-toolchain.toml"
                    | "deny.toml"
                    | ".github/workflows/rust.yml"
                    | "scripts/rust-foundation-smoke.sh"
            )
            || p.starts_with("docs/rust-rewrite/")
            || p.starts_with("docs/adr/")
    }) {
        let hash = command(root, "git", &["hash-object", "--", path], records)?;
        sources.push(json!({"path":path,"git_blob":hash.trim()}));
    }
    if sources.is_empty() {
        return Err("empty source snapshot".into());
    }
    Ok(sources)
}
fn source_check(root: &Path, id: &str, records: &mut Vec<Value>) -> Result<()> {
    let path = root.join(format!("crates/xtask/fixtures/{id}.json"));
    let f: Fixture = serde_json::from_slice(
        &fs::read(&path).map_err(|e| format!("missing fixture {}: {e}", path.display()))?,
    )
    .map_err(|e| format!("malformed fixture: {e}"))?;
    validate_fixture(&f, id)?;
    let tracked = command(root, "git", &["ls-files", "-z"], records)?;
    let required: BTreeSet<_> = tracked
        .split('\0')
        .filter(|p| {
            if id == "F01" {
                p.starts_with("docs/rust-rewrite/inventory/")
                    || p.ends_with(".py")
                    || p.ends_with(".pyw")
                    || p.ends_with(".pyi")
                    || *p == "ssx"
            } else {
                p.starts_with("docs/rust-rewrite/architecture/")
                    || p.starts_with("docs/adr/003-")
                    || p.starts_with("docs/adr/004-")
                    || p.starts_with("docs/adr/005-")
            }
        })
        .map(str::to_string)
        .collect();
    let provided: BTreeSet<_> = f.sources.iter().map(|s| s.path.clone()).collect();
    if required.is_empty() || required != provided {
        return Err("source fixture coverage is incomplete or unexpected".into());
    }

    for s in &f.sources {
        if id == "F01" && snapshot_fixture_amendment::f01(root, &s.path, &s.git_blob)?.is_some() {
            continue;
        }
        let actual = command(root, "git", &["hash-object", "--", &s.path], records)?;
        if actual.trim() != s.git_blob {
            return Err(format!("stale source fixture: {}", s.path));
        }
    }
    if id == "F01" {
        let manifest =
            load_json(&root.join("docs/rust-rewrite/inventory/python-disposition.json"))?;
        let rows = manifest["files"]
            .as_array()
            .ok_or("missing Python dispositions")?;
        let mut paths = BTreeSet::new();
        for row in rows {
            let p = row["path"].as_str().ok_or("missing disposition path")?;
            if !paths.insert(p.to_string())
                || row["disposition"].as_str().is_none_or(str::is_empty)
                || row["classification"].as_str().is_none_or(str::is_empty)
            {
                return Err("duplicate or unowned disposition".into());
            }
        }
        let tracked = command(root, "git", &["ls-files", "-z"], records)?;
        let expected: BTreeSet<String> = tracked
            .split('\0')
            .filter(|p| {
                p.ends_with(".py") || p.ends_with(".pyi") || p.ends_with(".pyw") || *p == "ssx"
            })
            .map(str::to_string)
            .collect();
        if paths.is_empty() || paths != expected {
            return Err("Python source inventory differs from tracked source set".into());
        }
        let pinned: BTreeSet<_> = f.sources.iter().map(|s| s.path.clone()).collect();
        if !expected.is_subset(&pinned) {
            return Err("fixture omits inventoried Python source".into());
        }
    }
    if id == "F02" {
        // ADR acceptance is defined by merged F02, not its proposal-heading wording.
        command(
            root,
            "git",
            &[
                "merge-base",
                "--is-ancestor",
                "562b6ce1ce9f798104311e8f3a213daefeddc795",
                "HEAD",
            ],
            records,
        )?;
        let pins = load_json(&root.join("docs/rust-rewrite/architecture/pins.json"))?;
        if pins["toolchain"]["rust"] != "1.99.0"
            || pins["direct_dependencies"]
                .as_object()
                .is_none_or(|o| o.is_empty())
        {
            return Err("architecture pins absent or changed".into());
        }
    }
    Ok(())
}
fn readiness(root: &Path, records: &mut Vec<Value>) -> Result<()> {
    let binary = root.join("target/debug/ssx-server");
    let binary_path = binary.to_str().ok_or("non-UTF8 binary path")?;
    let binary_hash = command(root, "git", &["hash-object", "--", binary_path], records)?;
    let mut child = Command::new(&binary)
        .args(["--listen", "127.0.0.1:0"])
        .env("PATH", "")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("native readiness spawn: {e}"))?;
    // Read startup on another thread; the timeout also catches a binary that never becomes ready.
    let stdout = child.stdout.take().ok_or("server stdout missing")?;
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut line = String::new();
        let result = BufReader::new(stdout).read_line(&mut line).map(|_| line);
        let _ = tx.send(result);
    });
    let result: Result<()> = (|| {
        let line = rx
            .recv_timeout(Duration::from_secs(10))
            .map_err(|_| "readiness startup timed out")?
            .map_err(|e| e.to_string())?;
        let startup: Value =
            serde_json::from_str(&line).map_err(|e| format!("invalid startup: {e}"))?;
        if startup["mode"] != "foundation" {
            return Err("wrong server mode".into());
        }
        let address = startup["address"]
            .as_str()
            .ok_or("missing listener address")?;
        let addr: std::net::SocketAddr = address.parse().map_err(|_| "invalid listener address")?;
        if !addr.ip().is_loopback() || addr.port() == 0 {
            return Err("listener is not bound loopback".into());
        }
        let mut stream =
            TcpStream::connect_timeout(&addr, Duration::from_secs(5)).map_err(|e| e.to_string())?;
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .map_err(|e| e.to_string())?;
        stream
            .write_all(b"GET /healthz HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
            .map_err(|e| e.to_string())?;
        let mut response = String::new();
        stream
            .read_to_string(&mut response)
            .map_err(|e| e.to_string())?;
        let (headers, body) = response
            .split_once("\r\n\r\n")
            .ok_or("invalid HTTP readiness response")?;
        let body: Value =
            serde_json::from_str(body).map_err(|e| format!("invalid readiness JSON: {e}"))?;
        if !headers.starts_with("HTTP/1.1 200 ")
            || body != json!({"status":"ok","mode":"foundation"})
        {
            return Err("readiness contract mismatch".into());
        }
        records.push(json!({"program":binary,"git_blob":binary_hash.trim(),"args":["--listen","127.0.0.1:0"],"environment":{"PATH":""},"startup":startup,"request":"GET /healthz","response":body,"status":"passed"}));
        Ok(())
    })();
    let _ = child.kill();
    let _ = child.wait();
    result?;
    let after_hash = command(root, "git", &["hash-object", "--", binary_path], records)?;
    if binary_hash != after_hash {
        return Err("readiness binary changed during smoke".into());
    }
    Ok(())
}
fn execute(root: &Path, id: &str, records: &mut Vec<Value>) -> Result<()> {
    let plan = load_json(&root.join("docs/rust-rewrite/plan.json"))?;
    let items = plan["items"].as_array().ok_or("malformed plan items")?;
    let mut ids = BTreeSet::new();
    for item in items {
        let item_id = item["id"].as_str().ok_or("malformed plan id")?;
        if item_id.is_empty() || !ids.insert(item_id) {
            return Err("empty/duplicate plan id".into());
        }
    }
    if !ids.contains(id) {
        return Err(format!("unknown plan id: {id}"));
    }
    match id {
        "A01" => a01::check(root, records),
        "A04" => a04::check(root, records),
        "A06" => a06::check(root, records),
        "A07" => a07::check(root, records),
        "A08" => a08::check(root, records),
        "A09" => a09::check(root, records),
        "A10" => a10::check(root, records),
        "A11" => a11::check(root, records),
        "A12" => a12::check(root, records),
        "A13" => a13::check(root, records),
        "A14" => a14::check(root, records),
        "A03" => a03::check(root, records),
        "A02" => a02::check(root, records),
        "A05" => a05::check(root, records),
        "O01" => o01::check(root, records),
        "O03" => o03::check(root, records),
        "O06" => o06::check(root, records),
        "F01" | "F02" => source_check(root, id, records),
        "F04" => replay::check(root, records),
        "T01" => t01::check(root, records),
        "T02" => t02::check(root, records),
        "T09" => t09::check(root, records),
        "T08A" => t08a::check(root, records),
        "T04" => t04::check(root, records),
        "T05" => t05::check(root, records),
        "T06" => t06::check(root, records),
        "F05" => f05::check(root, records),
        "F06" => f06::check(root, records),
        "F07" => f07::check(root, records),
        "W00" => w00::check(root, records),
        "W01" => w01::check(root, records),
        "W02" => w02::check(root, records),
        "W03" => w03::check(root, records),
        "W04" => w04::check(root, records),
        "W05" => w05::check(root, records),
        "W06" => w06::check(root, records),
        "W07" => w07::check(root, records),
        "W11" => w11::check(root, records),
        "F08" => f08::check(root, records),
        "F09" => f09::check(root, records),
        "F10" => f10::check(root, records),
        "F11" => f11::check(root, records),
        "F12" => f12::check(root, records),
        "F13" => f13::check(root, records),
        "F14" => f14::check(root, records),
        "F15" => f15::check(root, records),
        "F03" => {
            source_check(root, "F01", records)?;
            source_check(root, "F02", records)?;
            command(root, "cargo", &["fmt", "--all", "--check"], records)?;
            command(
                root,
                "cargo",
                &[
                    "clippy",
                    "--locked",
                    "--workspace",
                    "--all-targets",
                    "--exclude",
                    "ssx-cli-hyper",
                    "--",
                    "-D",
                    "warnings",
                ],
                records,
            )?;
            command(
                root,
                "cargo",
                &[
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
                ],
                records,
            )?;
            // xtask unit tests are run by the outer workspace gate; excluding this package prevents recursive invocation.
            command(
                root,
                "cargo",
                &[
                    "test",
                    "--locked",
                    "--workspace",
                    "--exclude",
                    "xtask",
                    "--features",
                    "ssx-cli-hyper/full,ssx-cli-hyper/source-client-framing",
                ],
                records,
            )?;
            command(
                root,
                "cargo",
                &["build", "--locked", "--workspace"],
                records,
            )?;
            readiness(root, records)
        }
        _ => Err(format!(
            "unimplemented acceptance checks for {id}; no owned executable fixtures"
        )),
    }
}
fn main() {
    if let Some(code) = t08a_child::fixture_entry() {
        std::process::exit(code);
    }
    if replay::cli() {
        return;
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root");
    let started_unix_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("wall clock after epoch")
        .as_millis();
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() == 2 && args[0] == "a06-source-cases" {
        if let Err(error) = a06::check_source_log(&root, Path::new(&args[1])) {
            eprintln!("{error}");
            std::process::exit(1);
        }
        println!("A06 exact completed source case inventory passed");
        return;
    }
    if args.as_slice() == ["a06-native-resource"] {
        let mut records = Vec::new();
        let result = a06_resource::check(&root, &mut records);
        println!("{}", serde_json::to_string_pretty(&json!({"status":if result.is_ok(){"passed"}else{"failed"},"commands":records,"error":result.as_ref().err()})).expect("native resource evidence"));
        if result.is_err() {
            std::process::exit(1);
        }
        return;
    }
    if args.first().is_some_and(|a| a == "ci-helper") {
        if let Err(error) = o06::helper(&root, &args[1..]) {
            eprintln!("{error}");
            std::process::exit(1);
        }
        return;
    }
    if args.first().is_some_and(|a| a == "verify-a09-transcript") {
        let result = if args.len() == 2 {
            a09::verify_transcript(&root, Path::new(&args[1]))
        } else {
            Err("usage: cargo xtask verify-a09-transcript PATH".into())
        };
        if let Err(error) = result {
            eprintln!("{error}");
            std::process::exit(1);
        }
        return;
    }
    let mut records = Vec::new();
    let id = args.get(1).map(String::as_str).unwrap_or("");
    let mut head_records = Vec::new();
    let head = command(&root, "git", &["rev-parse", "HEAD"], &mut head_records).unwrap_or_default();
    let tree = command(
        &root,
        "git",
        &["status", "--porcelain=v1", "--untracked-files=all"],
        &mut head_records,
    )
    .unwrap_or_default();
    let toolchain = command(
        &root,
        "rustc",
        &["--version", "--verbose"],
        &mut head_records,
    )
    .unwrap_or_default();
    let diff = command(
        &root,
        "git",
        &["diff", "--binary", "HEAD"],
        &mut head_records,
    )
    .unwrap_or_default();
    let snapshot = source_snapshot(&root, &mut head_records);
    let mut result = if args.len() != 2 && !(args.len() == 4 && args[2] == "--output")
        || args.first().map(String::as_str) != Some("check")
    {
        Err("usage: cargo xtask check <plan-id> [--output PATH]".into())
    } else {
        if head.is_empty() || toolchain.is_empty() || snapshot.is_err() {
            Err("source/toolchain identity unavailable".into())
        } else if let Err(error) = check_toolchain(&root, &toolchain) {
            Err(error)
        } else {
            if args.len() == 4 {
                validate_output(&root, Path::new(&args[3]))
                    .and_then(|_| execute(&root, id, &mut records))
            } else {
                execute(&root, id, &mut records)
            }
        }
    };
    if result.is_ok() {
        match source_snapshot(&root, &mut head_records) {
            Ok(after) if snapshot.as_ref().is_ok_and(|before| *before == after) => {}
            _ => {
                result = Err(
                    "source changed while acceptance checks ran; rerun against stable source"
                        .into(),
                )
            }
        }
    }
    let a08_scope = a08::report_scope(&records);
    let scope = if id == "T08A" {
        "standalone native external-HTTP CLI; finite source-bound loopback corpus, X509/proxy/error contract and source installation; native backend integration T08/T02 and production/O04 delivery remain pending"
    } else if id == "A12" {
        "static-source-bound infrastructure security factories and full native outputs/requests, actual private wire and finite ownership; no original execution/provider/full production composition claim"
    } else if id == "A10" {
        "source-bound five vulnerability adapters; full471 source observations/74 original test identities; exact reviewed differences and four correlated real-deadline IDs; private loopback/native helper; no production/W09/global RSS claim"
    } else if id == "A13" {
        "source-bound native DeHashed/HIBP/IntelX factories and Unicode primitives; 75 exact selectors, closed parent cases and three lossless private child lifecycles; no original Python/provider/full production/portal/global RSS claim"
    } else if id == "A11" {
        "source-bound threat-intelligence factories and full original tuples; actual private wire/ownership, closed F05/F06 differences; no providers/full production composition/global RSS"
    } else if id == "A09" {
        "source-bound catalog/media adapters; actual assertions plus guarded private loopback; qualification excludes incident calls and full portal/transport/global RSS"
    } else if id == "A05" {
        "offline source-bound DuckDuckGo/Google HTML adapter replay and loopback sessions; explicit host-only cookie correction; no live provider or deployment claims"
    } else if id == "A08" {
        a08_scope.as_str()
    } else if id == "A07" {
        "source-bound biomedical factories/config/scope,172 offline controls and charged runtime/context/compact regressions; no provider calls, global G02 or full HTML/MCP transport claim"
    } else if id == "A03" {
        "source-mapped GitHub, Hugging Face, Stack Exchange and Hacker News adapters through caller-owned F08/F10; offline loopback/private Valkey only, no provider or deployment claim"
    } else if id == "A02" {
        "source-bound pypi/rubygems/dockerhub adapters through registry and caller-owned F08 pool; no live providers, deployment or full server parity claim"
    } else if id == "A01" {
        "source-mapped npm/crates registry adapters through F06 factories and F08 loopback sessions; no live provider calls, deployment, performance or full adapter parity claim"
    } else if id == "A14" {
        "source-pinned Ashby/Greenhouse/Lever adapters through F08 loopback; company extraction, provider request/response handling, limits, failures, timeout and cancellation; zero provider calls"
    } else if id == "W00" {
        "saved definitions, optimistic revisions, tenant/index/quota authority and schedule ownership with private Valkey; no provider execution, comparison/report/outbox publication or public saved tools"
    } else if id == "W04" {
        "saved observations, comparisons and fenced atomic publication with private Valkey and native lifecycle qualification; no outbox, production dynamic grant reload or deployment claim"
    } else if id == "W05" {
        "saved event atomic publication, exact cursors, ack/replay/retention and actual loopback native MCP with private Valkey; typed observability seam only, full O01/W09 composition pending"
    } else if id == "W06" {
        "staged planning, owned canonical search/snapshot execution and fenced recovery with private Valkey/loopback; source qualification required; no production cutover or exactly-once remote dispatch claim"
    } else if id == "W07" {
        "exact native receipt/record/lineage/composition/MCP execution and measured frozen Python wire subset; no providers, production activation, startup or complete rewrite certification"
    } else if id == "O03" {
        "active tooling inventory and prospective evidence contracts only; no experiment, provider call, receipt issuance or historical evidence mutation"
    } else if id == "T01" {
        "synthetic MCP registry, real stdio/streamable HTTP/legacy SSE and gateway SDK qualification; no production tool parity or OAuth authorization-server claim"
    } else if id == "T06" {
        "source-bound native search/discovery through shared authenticated HTTP, stdio and legacy with real service/snapshot authorities; exactly two accepted outer-params diagnostic differences; production startup T02/W09 separate"
    } else if id == "T05" {
        "OAuth wire endpoints and all-network transport authentication with real clients/private Valkey; no provider, production cutover or browser identity claim"
    } else if id == "T04" {
        "authoritative machine OAuth state and atomic transitions with private Valkey; no endpoint, OIDC, provider or deployment claim"
    } else if id == "O06" {
        "additive CI inventory, native helpers and protected review trust preparation; pending-owner mappings are not semantic parity or cutover evidence"
    } else {
        "foundation checks only; no feature parity certification"
    };
    let mut evidence = json!
({"schema_version":1,"started_unix_ms":started_unix_ms,"finished_unix_ms":std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).expect("wall clock after epoch").as_millis(),"plan_id":id,"status":if result.is_ok(){"passed"}else{"failed"},"scope":scope,"source":{"head":head.trim(),"working_tree":tree,"diff":diff,"files":snapshot.unwrap_or_default()},"toolchain":toolchain,"identity_commands":head_records,"commands":records,"output_path":args.get(3),"error":result.err()});
    if args.len() == 4 && args[2] == "--output" {
        let path = PathBuf::from(&args[3]);
        let write = write_evidence(&root, &path, &evidence);
        if let Err(e) = write {
            evidence["status"] = json!("failed");
            evidence["error"] = json!(format!("evidence output {}: {e}", path.display()));
        }
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&evidence).expect("serializable evidence")
    );
    if evidence["status"] != "passed" {
        std::process::exit(1);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn compiler_identity_must_match_both_accepted_pins() {
        let pins = json!({"toolchain":{"rust":"1.99.0"}});
        let configuration = "[toolchain]\nchannel = \"1.99.0\"\n";
        let actual = "rustc 1.99.0 (abc123 2026-10-01)\nbinary: rustc\nrelease: 1.99.0\n";
        assert!(validate_toolchain(&pins, configuration, actual).is_ok());
        for invalid in [
            "".to_string(),
            "some nonempty output".to_string(),
            actual.replace("1.99.0", "1.98.0"),
            actual.replace("1.99.0", "1.99.0-nightly"),
            actual.replace("release: 1.99.0\n", ""),
            actual.replace("release: 1.99.0", "release: 1.98.0"),
            format!("{actual}release: 1.99.0\n"),
        ] {
            assert!(
                validate_toolchain(&pins, configuration, &invalid).is_err(),
                "{invalid}"
            );
        }
        for invalid in [json!({}), json!({"toolchain":{"rust":"1.98.0"}})] {
            assert!(validate_toolchain(&invalid, configuration, actual).is_err());
        }
        for invalid in [
            "",
            "[toolchain]",
            "[other]\nchannel = \"1.99.0\"",
            "[toolchain]\nchannel = \"1.98.0\"",
            "[toolchain]\nchannel = \"1.99.0\"\nchannel = \"1.99.0\"",
        ] {
            assert!(validate_toolchain(&pins, invalid, actual).is_err());
        }
    }
    #[test]
    fn output_cannot_overwrite_source_even_for_failure_reports() {
        let root = std::env::temp_dir().join(format!("ssx-output-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let root = root.canonicalize().unwrap();
        command(&root, "git", &["init", "--quiet"], &mut vec![]).unwrap();
        fs::write(root.join(".gitignore"), "target/\ncrates/ignored.rs\n").unwrap();
        fs::write(root.join("Cargo.toml"), "source bytes").unwrap();
        command(&root, "git", &["add", "Cargo.toml"], &mut vec![]).unwrap();
        fs::create_dir_all(root.join("crates")).unwrap();
        fs::write(root.join("crates/ignored.rs"), "owned bytes").unwrap();
        for status in ["passed", "failed"] {
            let report = json!({"status":status});
            for name in [
                "Cargo.toml",
                "crates/ignored.rs",
                ".git/config",
                "new-source.rs",
            ] {
                assert!(write_evidence(&root, &root.join(name), &report).is_err());
            }
            assert_eq!(
                fs::read_to_string(root.join("Cargo.toml")).unwrap(),
                "source bytes"
            );
            assert_eq!(
                fs::read_to_string(root.join("crates/ignored.rs")).unwrap(),
                "owned bytes"
            );
        }
        let output = root.join("target/evidence/report.json");
        write_evidence(&root, &output, &json!({"status":"passed"})).unwrap();
        assert_eq!(load_json(&output).unwrap()["status"], "passed");
        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            symlink(root.join("Cargo.toml"), root.join("target/alias.json")).unwrap();
            symlink(root.join("crates"), root.join("target/alias-dir")).unwrap();
            for name in ["target/alias.json", "target/alias-dir/new.rs"] {
                assert!(
                    write_evidence(&root, &root.join(name), &json!({"status":"failed"})).is_err()
                );
            }
            fs::hard_link(root.join("Cargo.toml"), root.join("target/hardlink.json")).unwrap();
            write_evidence(
                &root,
                &root.join("target/hardlink.json"),
                &json!({"status":"passed"}),
            )
            .unwrap();
            assert_eq!(
                fs::read_to_string(root.join("Cargo.toml")).unwrap(),
                "source bytes"
            );
        }
        let outside = root.with_extension("report.json");
        write_evidence(&root, &outside, &json!({"status":"passed"})).unwrap();
        fs::remove_file(outside).unwrap();
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn output_cannot_overwrite_linked_worktree_metadata() {
        let temporary =
            std::env::temp_dir().join(format!("ssx-linked-output-{}", std::process::id()));
        fs::create_dir_all(&temporary).unwrap();
        let temporary = temporary.canonicalize().unwrap();
        let primary = temporary.join("primary");
        let linked = temporary.join("linked");
        fs::create_dir_all(&primary).unwrap();
        command(&primary, "git", &["init", "--quiet"], &mut vec![]).unwrap();
        command(
            &primary,
            "git",
            &[
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@example.com",
                "-c",
                "commit.gpgsign=false",
                "commit",
                "--allow-empty",
                "-m",
                "initial",
            ],
            &mut vec![],
        )
        .unwrap();
        command(
            &primary,
            "git",
            &["worktree", "add", "--detach", linked.to_str().unwrap()],
            &mut vec![],
        )
        .unwrap();
        let directory = command(
            &linked,
            "git",
            &["rev-parse", "--absolute-git-dir"],
            &mut vec![],
        )
        .unwrap();
        let config = primary.join(".git/config");
        let head = Path::new(directory.trim()).join("HEAD");
        for path in [&config, &head] {
            assert!(!path.starts_with(&linked));
            let before = fs::read(path).unwrap();
            for status in ["passed", "failed"] {
                assert!(write_evidence(&linked, path, &json!({"status":status})).is_err());
                assert_eq!(fs::read(path).unwrap(), before);
            }
        }
        let output = temporary.join("report.json");
        write_evidence(&linked, &output, &json!({"status":"passed"})).unwrap();
        assert_eq!(load_json(&output).unwrap()["status"], "passed");
        fs::remove_dir_all(temporary).unwrap();
    }
    #[test]
    fn rejects_empty_wrong_and_duplicate_fixtures() {
        let mut f = Fixture {
            schema_version: 1,
            plan_id: "F01".into(),
            sources: vec![],
        };
        assert!(validate_fixture(&f, "F01").is_err());
        f.sources.push(Source {
            path: "foo".into(),
            git_blob: "0".repeat(40),
        });
        assert!(validate_fixture(&f, "F02").is_err());
        f.sources.push(Source {
            path: "foo".into(),
            git_blob: "0".repeat(40),
        });
        assert!(validate_fixture(&f, "F01").is_err());
    }
    #[test]
    fn rejects_escape_and_malformed() {
        let f = Fixture {
            schema_version: 1,
            plan_id: "F01".into(),
            sources: vec![Source {
                path: "../escape".into(),
                git_blob: "0".repeat(40),
            }],
        };
        assert!(validate_fixture(&f, "F01").is_err());
        assert!(serde_json::from_str::<Fixture>("{}").is_err());
    }
    #[test]
    fn unknown_and_unimplemented_fail() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        assert!(
            execute(&root, "T03", &mut vec![])
                .unwrap_err()
                .contains("unimplemented")
        );
        assert!(
            execute(&root, "INVALID", &mut vec![])
                .unwrap_err()
                .contains("unknown")
        );
    }
    #[test]
    fn missing_empty_malformed_stale_and_incomplete_fail() {
        let root = std::env::temp_dir().join(format!("ssx-xtask-negative-{}", std::process::id()));
        fs::create_dir_all(root.join("crates/xtask/fixtures")).unwrap();
        let fixture = root.join("crates/xtask/fixtures/F01.json");
        let mut records = vec![];
        assert!(
            source_check(&root, "F01", &mut records)
                .unwrap_err()
                .contains("missing fixture")
        );
        fs::write(&fixture, "{}").unwrap();
        assert!(
            source_check(&root, "F01", &mut records)
                .unwrap_err()
                .contains("malformed fixture")
        );
        fs::write(
            &fixture,
            r#"{"schema_version":1,"plan_id":"F01","sources":[]}"#,
        )
        .unwrap();
        assert!(
            source_check(&root, "F01", &mut records)
                .unwrap_err()
                .contains("empty fixture")
        );
        command(&root, "git", &["init", "--quiet"], &mut records).unwrap();
        fs::write(root.join("example.py"), "# real source\n").unwrap();
        command(&root, "git", &["add", "example.py"], &mut records).unwrap();
        fs::write(&fixture,serde_json::to_vec(&json!({"schema_version":1,"plan_id":"F01","sources":[{"path":"example.py","git_blob":"0".repeat(40)}]})).unwrap()).unwrap();
        assert!(
            source_check(&root, "F01", &mut records)
                .unwrap_err()
                .contains("stale source")
        );
        fs::write(&fixture,serde_json::to_vec(&json!({"schema_version":1,"plan_id":"F01","sources":[{"path":"other.py","git_blob":"0".repeat(40)}]})).unwrap()).unwrap();
        assert!(
            source_check(&root, "F01", &mut records)
                .unwrap_err()
                .contains("coverage")
        );
        fs::create_dir_all(root.join("docs/rust-rewrite")).unwrap();
        fs::write(
            root.join("docs/rust-rewrite/plan.json"),
            r#"{"items":[{"id":"F01"},{"id":"F01"}]}"#,
        )
        .unwrap();
        assert!(
            execute(&root, "F01", &mut records)
                .unwrap_err()
                .contains("duplicate")
        );
        fs::remove_dir_all(root).unwrap();
    }
}
