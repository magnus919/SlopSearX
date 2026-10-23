//! Developer build/stage/exec path; installed users need no helper configuration.
use std::os::unix::process::CommandExt;
use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
};
// Cargo metadata defines the target root; current_exe only selects its exact
// profile/optional explicit target. Never infer target-dir by counting parents.
fn build_layout(target: &Path, exe: &Path) -> Result<(bool, Option<String>), String> {
    let relative = exe
        .strip_prefix(target)
        .map_err(|_| "xtask outside Cargo metadata target directory")?;
    let parts: Vec<_> = relative
        .components()
        .map(|p| p.as_os_str().to_str().ok_or("non-UTF8 Cargo layout"))
        .collect::<Result<_, _>>()?;
    let (triple, profile, role) = match parts.as_slice() {
        [profile, role] => (None, *profile, *role),
        [triple, profile, role] if !triple.is_empty() => {
            (Some((*triple).to_owned()), *profile, *role)
        }
        _ => return Err("unsupported Cargo executable layout".into()),
    };
    if role != "xtask" || !matches!(profile, "debug" | "release") {
        return Err("expected Cargo xtask debug/release role".into());
    }
    Ok((profile == "release", triple))
}
fn artifact(
    values: &[serde_json::Value],
    package: &str,
    name: &str,
    source: &Path,
) -> Result<PathBuf, String> {
    let finished: Vec<_> = values
        .iter()
        .filter(|v| v["reason"] == "build-finished")
        .collect();
    if finished.len() != 1 || finished[0]["success"] != true {
        return Err("build not uniquely successful".into());
    }
    let found: Vec<_> = values
        .iter()
        .filter(|v| {
            v["reason"] == "compiler-artifact"
                && v["package_id"].as_str() == Some(package)
                && v["target"]["name"] == name
                && v["target"]["src_path"].as_str() == source.to_str()
                && v["target"]["kind"] == serde_json::json!(["bin"])
                && v["profile"]["test"] == false
                && v["executable"].is_string()
        })
        .collect();
    if found.len() != 1 {
        return Err("build artifact absent/ambiguous".into());
    }
    let path = PathBuf::from(
        found[0]["executable"]
            .as_str()
            .ok_or("artifact path absent")?,
    );
    if !path.is_absolute() {
        return Err("artifact not absolute".into());
    }
    Ok(path)
}
fn package(root: &Path, distribution: bool) -> Result<PathBuf, String> {
    let cargo = PathBuf::from(env!("CARGO"));
    if !cargo.is_absolute() {
        return Err("pinned Cargo unavailable".into());
    }
    let metadata = Command::new(&cargo)
        .current_dir(root)
        .args([
            "metadata",
            "--offline",
            "--locked",
            "--no-deps",
            "--format-version=1",
        ])
        .output()
        .map_err(|_| "Cargo metadata failed")?;
    if !metadata.status.success() {
        return Err("Cargo metadata failed".into());
    }
    let metadata: serde_json::Value =
        serde_json::from_slice(&metadata.stdout).map_err(|_| "Cargo metadata invalid")?;
    let target = PathBuf::from(
        metadata["target_directory"]
            .as_str()
            .ok_or("Cargo target directory absent")?,
    )
    .canonicalize()
    .map_err(|_| "existing Cargo target absent")?;
    let exe = std::env::current_exe()
        .and_then(|p| p.canonicalize())
        .map_err(|_| "xtask identity unavailable")?;
    let (release, triple) = build_layout(&target, &exe)?;
    let mut command = Command::new(&cargo);
    command
        .current_dir(root)
        .args([
            "build",
            "--offline",
            "--locked",
            "-p",
            "ssx-gateway",
            "--bin",
            "ssx-gateway",
            "--bin",
            "ssx-gateway-dns",
            "--message-format=json",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
    // Pin the metadata target root and selected executable target explicitly:
    // config/CARGO_BUILD_TARGET cannot append a second target directory.
    command.arg("--target-dir").arg(&target);
    if let Some(triple) = triple {
        command.args(["--target", &triple]);
    }
    if release {
        command.arg("--release");
    }
    let output = command
        .output()
        .map_err(|_| "gateway package build failed")?;
    if !output.status.success() {
        return Err("gateway package build failed".into());
    }
    let values: Vec<serde_json::Value> = String::from_utf8(output.stdout)
        .map_err(|_| "compiler JSON encoding")?
        .lines()
        .filter(|s| !s.trim().is_empty())
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()
        .map_err(|_| "compiler JSON invalid")?;
    let packages = metadata["packages"]
        .as_array()
        .ok_or("Cargo package identities absent")?;
    let mut outputs = Vec::new();
    for (name, source) in [
        ("ssx-gateway", "crates/ssx-gateway/src/main.rs"),
        (
            "ssx-gateway-dns",
            "crates/ssx-gateway/src/bin/ssx-gateway-dns.rs",
        ),
    ] {
        let manifest = root.join("crates/ssx-gateway/Cargo.toml");
        let ids: Vec<_> = packages
            .iter()
            .filter(|v| {
                v["name"] == "ssx-gateway" && v["manifest_path"].as_str() == manifest.to_str()
            })
            .collect();
        if ids.len() != 1 {
            return Err("Cargo package identity ambiguous".into());
        }
        let path = artifact(
            &values,
            ids[0]["id"].as_str().ok_or("package id absent")?,
            name,
            &root.join(source),
        )?;
        if !path.starts_with(&target) || !path.is_file() || path.is_symlink() {
            return Err("actual build artifact invalid".into());
        }
        outputs.push(path);
    }
    if outputs[0].parent() != outputs[1].parent()
        || outputs[1]
            .file_name()
            .is_none_or(|n| n != "ssx-gateway-dns")
    {
        return Err("actual package artifacts not siblings".into());
    }
    let output_root = target.join(if distribution {
        "gateway-distributions"
    } else {
        "gateway-packages"
    });
    match std::fs::create_dir(&output_root) {
        Ok(()) => {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&output_root, std::fs::Permissions::from_mode(0o700))
                .map_err(|_| "private package root")?;
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(_) => return Err("private package root".into()),
    }
    (if distribution {
        ssx_mcp::gateway_client::resolver::install::stage_distribution(
            &outputs[0],
            &outputs[1],
            &output_root,
        )
    } else {
        ssx_mcp::gateway_client::resolver::install::stage(&outputs[0], &outputs[1], &output_root)
    })
    .map_err(|_| "gateway package staging/identity failed".into())
}
/// Runs before evidence-runner git/probe commands, so MCP stdout stays clean.
pub fn cli(root: &Path, args: &[String]) -> bool {
    let Some(role) = args.first() else {
        return false;
    };
    if !matches!(role.as_str(), "gateway" | "gateway-package") {
        return false;
    }
    if role == "gateway-package" && args.len() != 1 {
        eprintln!("usage: cargo xtask gateway-package");
        std::process::exit(1);
    }
    let executable = match package(root, role == "gateway-package") {
        Ok(path) => path,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1)
        }
    };
    if role == "gateway-package" {
        println!(
            "{}",
            executable
                .parent()
                .and_then(Path::parent)
                .expect("validated package")
                .display()
        );
        return true;
    }
    // Replace the developer wrapper: no forwarding child/supervisor is detached.
    // Exact gateway arguments and environment remain unchanged.
    let mut rest = &args[1..];
    if rest.first().is_some_and(|a| a == "--") {
        rest = &rest[1..];
    }
    let _error = Command::new(executable).args(rest).exec();
    eprintln!("installed gateway execution failed");
    std::process::exit(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn explicit_target_layout_does_not_duplicate_target_directory() {
        let root = Path::new("/owned/target");
        assert_eq!(
            build_layout(
                root,
                Path::new("/owned/target/aarch64-apple-darwin/debug/xtask")
            )
            .unwrap(),
            (false, Some("aarch64-apple-darwin".into()))
        );
        assert_eq!(
            build_layout(root, Path::new("/owned/target/release/xtask")).unwrap(),
            (true, None)
        );
        assert!(build_layout(root, Path::new("/other/debug/xtask")).is_err());
        assert!(build_layout(root, Path::new("/owned/target/a/b/debug/xtask")).is_err());
    }
    fn output() -> Vec<serde_json::Value> {
        vec![
            json!({"reason":"compiler-artifact","package_id":"exact-id","target":{"name":"ssx-gateway","src_path":"/source/main.rs","kind":["bin"]},"profile":{"test":false},"executable":"/owned/target/triple/debug/ssx-gateway"}),
            json!({"reason":"build-finished","success":true}),
        ]
    }
    #[test]
    fn actual_artifact_selection_ignores_stale_sibling_and_binds_all_roles() {
        let source = Path::new("/source/main.rs");
        assert_eq!(
            artifact(&output(), "exact-id", "ssx-gateway", source).unwrap(),
            Path::new("/owned/target/triple/debug/ssx-gateway")
        );
        assert!(artifact(&output(), "stale-package", "ssx-gateway", source).is_err());
        assert!(
            artifact(
                &output(),
                "exact-id",
                "ssx-gateway",
                Path::new("/wrong/main.rs")
            )
            .is_err()
        );
        for (field, value) in [
            ("reason", json!("compiler-message")),
            ("profile", json!({"test":true})),
            (
                "target",
                json!({"name":"ssx-gateway","src_path":"/source/main.rs","kind":["test"]}),
            ),
            ("executable", json!("relative/stale")),
        ] {
            let mut values = output();
            values[0][field] = value;
            assert!(artifact(&values, "exact-id", "ssx-gateway", source).is_err());
        }
        let mut duplicate = output();
        duplicate.push(duplicate[0].clone());
        assert!(artifact(&duplicate, "exact-id", "ssx-gateway", source).is_err());
        let mut failed = output();
        failed[1]["success"] = json!(false);
        assert!(artifact(&failed, "exact-id", "ssx-gateway", source).is_err());
        assert!(artifact(&output()[..1], "exact-id", "ssx-gateway", source).is_err());
    }
}
