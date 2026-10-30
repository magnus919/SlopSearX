//! Finite CLI-only X509 dependency exception; backend Rustls policy is unchanged.
use crate::{Result, command};
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
const EXCLUDED: [&str; 4] = [
    "ssx-cli",
    "ssx-cli-check",
    "ssx-cli-openssl-store",
    "ssx-cli-hyper",
];
const BANNED: [&str; 3] = ["native-tls", "hyper-tls", "openssl-sys"];
const PINS: [(&str, &str, &str); 3] = [
    (
        "openssl",
        "0.10.81",
        "77823a27f0babb03091cb9ed9ef80af3b39dbc82f97e8fa530374b7dafd87a45",
    ),
    (
        "openssl-sys",
        "0.9.117",
        "b47e7e6bb2c38cd930d25a23b40fa52e068c10e85f3e03a7f5ba5aaca5713695",
    ),
    (
        "openssl-src",
        "300.6.1+3.6.3",
        "46eb8fb9fb3b61ce1c0f8a026c4c1a0714d3a9e138e7fbde78753ce2babc3846",
    ),
];
fn need(ok: bool, msg: &str) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(format!("T08A TLS roles: {msg}"))
    }
}
fn names(tree: &str) -> BTreeSet<&str> {
    tree.lines()
        .filter_map(|l| l.split_whitespace().next())
        .collect()
}
fn backend_ok(tree: &str) -> bool {
    let names = names(tree);
    BANNED.iter().all(|n| !names.contains(n))
}
fn backend(root: &Path, host: &str, records: &mut Vec<Value>) -> Result<String> {
    command(
        root,
        "cargo",
        &[
            "tree",
            "--locked",
            "--workspace",
            "--exclude",
            EXCLUDED[0],
            "--exclude",
            EXCLUDED[1],
            "--exclude",
            EXCLUDED[2],
            "--exclude",
            EXCLUDED[3],
            "--edges",
            "features",
            "--prefix",
            "none",
            "--target",
            &host,
        ],
        records,
    )
}
fn pins(root: &Path, cli: &str) -> Result<()> {
    let lock = fs::read_to_string(root.join("Cargo.lock")).map_err(|e| e.to_string())?;
    for (name, version, checksum) in PINS {
        let blocks = lock
            .split("[[package]]")
            .filter(|p| p.lines().any(|l| l == format!("name = \"{name}\"")))
            .collect::<Vec<_>>();
        need(
            blocks.len() == 1,
            "no parallel/unknown OpenSSL package allowed",
        )?;
        for line in [
            format!("version = \"{version}\""),
            format!("checksum = \"{checksum}\""),
            "source = \"registry+https://github.com/rust-lang/crates.io-index\"".into(),
        ] {
            need(
                blocks[0].lines().any(|l| l == line),
                "exact OpenSSL registry identity/checksum",
            )?;
        }
        let actual = cli
            .lines()
            .filter_map(|l| {
                let mut p = l.split_whitespace();
                let n = p.next()?;
                let v = p.next()?;
                (n == name && v.starts_with('v')).then(|| v.trim_start_matches('v'))
            })
            .collect::<BTreeSet<_>>();
        need(
            actual == BTreeSet::from([version]),
            "exact CLI-only OpenSSL graph version",
        )?;
    }
    need(
        cli.lines().any(|l| l == "openssl feature \"vendored\"")
            && cli.lines().any(|l| l == "openssl-sys feature \"vendored\""),
        "static vendored chain required",
    )?;
    need(
        !names(cli).contains("native-tls") && !names(cli).contains("hyper-tls"),
        "no alternate CLI TLS backend",
    )
}
struct Probe(PathBuf);
impl Drop for Probe {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
impl Probe {
    fn new(target: &Path) -> Result<Self> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_nanos();
        for n in 0..32 {
            let p = target
                .join("rust-evidence")
                .join(format!("tls-role-probe-{}-{now}-{n}", std::process::id()));
            fs::create_dir_all(p.parent().ok_or("probe parent")?).map_err(|e| e.to_string())?;
            match fs::create_dir(&p) {
                Ok(()) => return Ok(Self(p)),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e.to_string()),
            }
        }
        Err("owned role-probe collision budget".into())
    }
}
/// This is graph/provenance acceptance only; never native backend/TLS execution.
pub(crate) fn check(root: &Path, records: &mut Vec<Value>) -> Result<()> {
    let host = super::native_host(root, records)?;
    let back = backend(root, &host, records)?;
    need(
        names(&back).contains("rustls-platform-verifier"),
        "original backend Rustls sentinel missing",
    )?;
    need(
        backend_ok(&back),
        "forbidden TLS package reachable from selected backend",
    )?;
    let cli = command(
        root,
        "cargo",
        &[
            "tree",
            "--locked",
            "-p",
            "ssx-cli",
            "--edges",
            "normal,build,features",
            "--prefix",
            "none",
            "--target",
            &host,
        ],
        records,
    )?;
    pins(root, &cli)?;
    let checker = command(
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
            "--target",
            &host,
        ],
        records,
    )?;
    need(
        names(&checker)
            .iter()
            .all(|n| !n.starts_with("ssx-") || *n == "ssx-cli-check"),
        "checker must not link CLI/backend crates",
    )?;
    need(
        !names(&checker).iter().any(|n| {
            n.starts_with("openssl")
                || ["libxml", "native-tls", "hyper-tls", "pyo3", "cpython"].contains(n)
        }),
        "checker execution graph must remain backend/crypto/interpreter free",
    )?;
    let meta = command(
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
    let meta: Value = serde_json::from_str(&meta).map_err(|e| e.to_string())?;
    let target = PathBuf::from(
        meta["target_directory"]
            .as_str()
            .ok_or("target directory")?,
    );
    let probe = Probe::new(&target)?;
    fs::create_dir(probe.0.join("src")).map_err(|e| e.to_string())?;
    let store = serde_json::to_string(
        root.join("crates/ssx-cli-openssl-store")
            .to_str()
            .ok_or("source path UTF8")?,
    )
    .map_err(|e| e.to_string())?;
    let manifest = format!(
        "[workspace]\nmembers = [\".\"]\n[package]\nname = \"backend-role-probe\"\nversion = \"0.0.0\"\nedition = \"2024\"\nrust-version = \"1.99\"\n[dependencies]\nssx-cli-openssl-store = {{ path = {store} }}\n"
    );
    fs::write(probe.0.join("Cargo.toml"), &manifest).map_err(|e| e.to_string())?;
    fs::write(probe.0.join("src/lib.rs"), "pub fn no_execution() {}\n")
        .map_err(|e| e.to_string())?;
    let mut seeded = fs::read_to_string(root.join("Cargo.lock")).map_err(|e| e.to_string())?;
    seeded.push_str("\n[[package]]\nname = \"backend-role-probe\"\nversion = \"0.0.0\"\ndependencies = [\n \"ssx-cli-openssl-store\",\n]\n");
    fs::write(probe.0.join("Cargo.lock"), seeded).map_err(|e| e.to_string())?;
    let manifest_path = probe.0.join("Cargo.toml");
    // Only the registered throwaway probe lock may prune. The real project lock
    // stays --locked in every role query and is checked by source snapshots.
    let graph = command(
        root,
        "cargo",
        &[
            "tree",
            "--offline",
            "--manifest-path",
            manifest_path.to_str().ok_or("probe path UTF8")?,
            "--workspace",
            "--exclude",
            EXCLUDED[0],
            "--exclude",
            EXCLUDED[1],
            "--exclude",
            EXCLUDED[2],
            "--exclude",
            EXCLUDED[3],
            "--edges",
            "features",
            "--prefix",
            "none",
            "--target",
            &host,
        ],
        records,
    )?;
    need(
        names(&graph).contains("backend-role-probe")
            && names(&graph).contains("ssx-cli-openssl-store")
            && !backend_ok(&graph),
        "actual transitive forbidden dependency must survive root exclusion and be rejected",
    )?;
    let path = probe.0.clone();
    drop(probe);
    need(!path.exists(), "owned graph probe retired")?;
    records.push(json!({"plan":"T08A-TLS-ROLES","scope":"static locked backend/CLI/checker graph and exact CLI-only vendored OpenSSL identities; no runtime/backend qualification","excluded_workspace_roots":EXCLUDED,"backend_forbidden":BANNED,"private_cli_pins":PINS,"actual_transitive_negative_rejected":true,"owned_probe_retired":true}));
    Ok(())
}
