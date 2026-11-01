//! Native O02 image-promotion qualification; full O02 remains incomplete until
//! all O02-owned backend Python tooling and tests have accepted successors.
use serde_json::Value;
use std::{fs, path::Path};
type Result<T> = std::result::Result<T, String>;

fn active_python_sources(root: &Path) -> Result<Vec<String>> {
    let path = root.join("docs/rust-rewrite/inventory/python-disposition.json");
    let manifest: Value =
        serde_json::from_slice(&fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?)
            .map_err(|e| format!("{}: {e}", path.display()))?;
    let rows = manifest["files"]
        .as_array()
        .ok_or("Python disposition inventory missing files")?;
    let mut active = Vec::new();
    for row in rows {
        if row["classification"] != "backend-coupled tooling/test"
            || !row["migration_owners"]
                .as_array()
                .is_some_and(|owners| owners.iter().any(|owner| owner.as_str() == Some("O02")))
        {
            continue;
        }
        let source = row["path"]
            .as_str()
            .ok_or("Python disposition path missing")?;
        if !source.ends_with(".py") && !source.ends_with(".pyw") && !source.ends_with(".pyi") {
            continue;
        }
        if fs::symlink_metadata(root.join(source)).is_ok() {
            active.push(source.to_string());
        }
    }
    active.sort();
    Ok(active)
}

pub(crate) fn check(root: &Path, records: &mut Vec<Value>) -> Result<()> {
    // Authenticate the O02-owned retirement closure without coupling this
    // lane to unrelated F01 successors such as the O03 tooling inventory.
    let fixture = fs::read(root.join("crates/xtask/fixtures/F01.json"))
        .map_err(|e| format!("F01 historical disposition fixture: {e}"))?;
    let tracked = super::command(root, "git", &["ls-files", "-z"], records)?
        .split('\0')
        .filter(|path| !path.is_empty())
        .map(str::to_string)
        .collect();
    super::o02_health_retirement::authenticate(root, &fixture, &tracked)?;
    super::command(root, "cargo", &["fmt", "--all", "--check"], records)?;
    super::command(
        root,
        "cargo",
        &[
            "test",
            "--locked",
            "--offline",
            "-p",
            "ssx-ops",
            "--all-targets",
            "--",
            "--test-threads=1",
        ],
        records,
    )?;
    super::command(
        root,
        "cargo",
        &[
            "clippy",
            "--locked",
            "--offline",
            "-p",
            "ssx-ops",
            "--all-targets",
            "--",
            "-D",
            "warnings",
        ],
        records,
    )?;
    super::command(
        root,
        "cargo",
        &["test", "--locked", "--offline", "-p", "xtask", "o02::tests"],
        records,
    )?;

    let pending = active_python_sources(root)?;
    if !pending.is_empty() {
        return Err(format!(
            "O02 incomplete: O02-owned backend Python sources still require native successor evidence: {}",
            pending.join(", ")
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT: AtomicU64 = AtomicU64::new(0);

    struct Scratch(std::path::PathBuf);
    impl Scratch {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "ssx-o02-gate-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&root).unwrap();
            Self(root)
        }
    }
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn reports_only_present_o02_owned_backend_python_sources() {
        let scratch = Scratch::new();
        let inventory = scratch.0.join("docs/rust-rewrite/inventory");
        fs::create_dir_all(&inventory).unwrap();
        fs::create_dir_all(scratch.0.join("scripts")).unwrap();
        fs::write(scratch.0.join("scripts/owned.py"), b"pass\n").unwrap();
        let manifest = json!({"files":[
            {"path":"scripts/owned.py","classification":"backend-coupled tooling/test","migration_owners":["O02"]},
            {"path":"scripts/other.py","classification":"backend-coupled tooling/test","migration_owners":["O03"]},
            {"path":"scripts/client.py","classification":"client-only","migration_owners":["O02"]}
        ]});
        fs::write(
            inventory.join("python-disposition.json"),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
        assert_eq!(
            active_python_sources(&scratch.0).unwrap(),
            ["scripts/owned.py"]
        );
        fs::remove_file(scratch.0.join("scripts/owned.py")).unwrap();
        assert!(active_python_sources(&scratch.0).unwrap().is_empty());
    }

    #[test]
    fn o02_source_admission_mutation_controls_pass() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        super::super::o02_health_retirement::controls(&root).unwrap();
    }
}
