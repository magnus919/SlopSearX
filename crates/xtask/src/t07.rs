//! Source-bound actual native gateway qualification. Never executes Python.
use crate::{Result, command};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, fs, path::Path};
const BINDINGS: &str = "d35d65b6eb37ed92c1dc832f4df2fa989f7d398ac95a90235eb3a1ba5d69028c";
fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|v| format!("{v:02x}"))
        .collect()
}
fn validate(raw: &[u8]) -> Result<Value> {
    if digest(raw) != BINDINGS {
        return Err("T07 reviewed source/test identities changed".into());
    }
    let value: Value = serde_json::from_slice(raw).map_err(|e| e.to_string())?;
    if value["schema_version"] != 1
        || value["oracle_execution"]
            != "none; source-derived contracts plus retained component captures"
    {
        return Err("T07 contract provenance invalid".into());
    }
    Ok(value)
}
fn suite(
    root: &Path,
    package: &str,
    target: &str,
    filter: Option<&str>,
    expected: &[Value],
    records: &mut Vec<Value>,
) -> Result<()> {
    let mut args = vec!["test", "--offline", "--locked", "-p", package];
    if target == "--lib" {
        args.push("--lib")
    } else {
        args.extend(["--test", target])
    }
    if target == "t07_dns_owner" {
        args.extend(["--features", "dns-test-fixtures"]);
    }
    if target == "t07_source_stages" {
        args.extend(["--features", "source-endpoint-test-fixtures"]);
    }
    if package == "ssx-mcp" && target == "--lib" {
        args.extend(["--features", "gateway-source-endpoints"]);
    }
    if let Some(filter) = filter {
        args.push(filter)
    }
    let mut listing = args.clone();
    listing.extend(["--", "--list"]);
    let output = command(root, "cargo", &listing, records)?;
    let actual: BTreeSet<_> = output
        .lines()
        .filter_map(|line| line.strip_suffix(": test"))
        .collect();
    let names: BTreeSet<_> = expected
        .iter()
        .map(|v| v.as_str().ok_or("T07 test selector invalid"))
        .collect::<std::result::Result<_, _>>()?;
    if actual != names || names.len() != expected.len() {
        return Err(format!(
            "T07 exact compiled test inventory differs: {target}"
        ));
    }
    args.extend(["--", "--test-threads=1"]);
    let output = command(root, "cargo", &args, records)?;
    if !output.contains(&format!(
        "test result: ok. {} passed; 0 failed; 0 ignored;",
        expected.len()
    )) {
        return Err(format!("T07 ignored/missing/failed tests: {target}"));
    }
    Ok(())
}
// Complete owned namespace membership, including hidden files. Source trees
// cannot silently acquire generated markers, links, caches or special files.
fn vendor_members(root: &Path, directory: &Path, members: &mut BTreeSet<String>) -> Result<()> {
    for entry in fs::read_dir(directory).map_err(|e| e.to_string())? {
        let path = entry.map_err(|e| e.to_string())?.path();
        let metadata = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
        if metadata.file_type().is_symlink() {
            return Err("T07 vendor symlink refused".into());
        }
        if metadata.is_dir() {
            vendor_members(root, &path, members)?;
        } else if metadata.is_file() {
            members.insert(
                path.strip_prefix(root)
                    .map_err(|e| e.to_string())?
                    .to_str()
                    .ok_or("T07 non-UTF8 vendor path")?
                    .to_owned(),
            );
        } else {
            return Err("T07 vendor special file refused".into());
        }
    }
    Ok(())
}
pub fn check(root: &Path, records: &mut Vec<Value>) -> Result<()> {
    if !cfg!(unix) {
        return Err("T07 native qualification inventory is currently Linux/macOS; no Windows acceptance claim".into());
    }
    let install = std::env::var_os("SLOPSEARX_INSTALL_TEST_ROOT")
        .ok_or("T07 controller must preregister a private install fixture root")?;
    let install = Path::new(&install);
    if !install.is_absolute() || !install.is_dir() || install.is_symlink() {
        return Err("T07 private install fixture root invalid".into());
    }
    let value = validate(
        &fs::read(root.join("docs/rust-rewrite/t07/current-source-bindings.json"))
            .map_err(|e| e.to_string())?,
    )?;
    let mut seen = BTreeSet::new();
    for row in value["files"]
        .as_array()
        .ok_or("T07 source inventory absent")?
    {
        let path = row["path"].as_str().ok_or("T07 source path absent")?;
        if Path::new(path).is_absolute() || path.split('/').any(|v| v == "..") || !seen.insert(path)
        {
            return Err("T07 unsafe/duplicate source path".into());
        }
        let bytes = fs::read(root.join(path)).map_err(|e| format!("T07 {path}: {e}"))?;
        if row["sha256"] != digest(&bytes) {
            return Err(format!("T07 reviewed source drift: {path}"));
        }
    }
    let expected_vendor: BTreeSet<_> = seen
        .iter()
        .filter(|path| {
            path.starts_with("vendor/ssx-http-source/") || path.starts_with("vendor/ssx-sdk-auth/")
        })
        .map(|path| (*path).to_owned())
        .collect();
    let mut actual_vendor = BTreeSet::new();
    for namespace in ["vendor/ssx-http-source", "vendor/ssx-sdk-auth"] {
        let directory = root.join(namespace);
        if fs::symlink_metadata(&directory)
            .map_err(|e| e.to_string())?
            .file_type()
            .is_symlink()
        {
            return Err("T07 vendor namespace symlink refused".into());
        }
        vendor_members(root, &directory, &mut actual_vendor)?;
    }
    if actual_vendor != expected_vendor
        || actual_vendor
            .iter()
            .any(|path| path.ends_with("/.cargo-ok"))
    {
        return Err("T07 complete vendor membership differs or contains generated markers".into());
    }
    let unit_path = std::env::var_os("SLOPSEARX_T07_UNIT_PROFILE_ROOT")
        .ok_or("T07 requires a preregistered external unit profile root")?;
    let unit_path = Path::new(&unit_path);
    let unit = crate::t07_unit_profile::Profile::open(root, unit_path, unit_path.exists())?;
    // Every selected project target and the genuine unit root compile before any runtime case.
    unit.compile(true, records)?;
    // Compilation itself proves the real concrete fork APIs are active, not a
    // marker string or a synthetic substitute in the executable.
    command(
        root,
        "cargo",
        &[
            "build",
            "--offline",
            "--locked",
            "-p",
            "ssx-gateway",
            "--bin",
            "ssx-gateway",
            "--bin",
            "ssx-gateway-dns",
        ],
        records,
    )?;
    suite(
        root,
        "ssx-mcp",
        "--lib",
        Some("gateway_client::"),
        value["library_tests"]
            .as_array()
            .ok_or("T07 library inventory absent")?,
        records,
    )?;
    suite(
        root,
        "ssx-mcp",
        "t07_streaming_seam",
        None,
        value["seam_tests"]
            .as_array()
            .ok_or("T07 seam inventory absent")?,
        records,
    )?;
    suite(
        root,
        "ssx-gateway",
        "t07_runtime",
        None,
        value["process_tests"]
            .as_array()
            .ok_or("T07 process inventory absent")?,
        records,
    )?;
    suite(
        root,
        "ssx-mcp",
        "--lib",
        Some("opaque::tests::owned_stdio_"),
        value["stdio_owner_tests"]
            .as_array()
            .ok_or("T07 stdio owner inventory absent")?,
        records,
    )?;
    suite(
        root,
        "ssx-gateway",
        "t07_dns_owner",
        None,
        value["dns_owner_tests"]
            .as_array()
            .ok_or("T07 DNS owner inventory absent")?,
        records,
    )?;
    suite(
        root,
        "ssx-gateway",
        "t07_source_stages",
        None,
        value["source_stage_tests"]
            .as_array()
            .ok_or("T07 source stage inventory absent")?,
        records,
    )?;
    unit.tests(
        value["source_pool_unit_tests"]
            .as_array()
            .ok_or("T07 source-pool unit inventory absent")?,
        records,
    )?;
    let count = |key: &str| -> Result<usize> {
        value[key]
            .as_array()
            .map(Vec::len)
            .ok_or_else(|| format!("T07 {key} inventory absent"))
    };
    let focused_native_cases = count("library_tests")?
        + count("seam_tests")?
        + count("process_tests")?
        + count("dns_owner_tests")?
        + count("source_pool_unit_tests")?
        + count("source_stage_tests")?;
    let stdio_owner_controls = count("stdio_owner_tests")?;
    records.push(json!({"contract":"native_gateway_source_bound","source_files":seen.len(),"focused_native_cases":focused_native_cases,"stdio_owner_controls":stdio_owner_controls,"dns_owner_controls":count("dns_owner_tests")?,"focused_native_selectors_total":focused_native_cases+stdio_owner_controls,"python_executed":false,"provider_calls":0,"full_transport_component_acceptance":"independent R1/R2/profile/SDK source-bound receipts and inherited T01/T05/T06/F08 remain required"}));
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reviewed_t07_binding_rejects_single_byte_mutation_and_missing_inventory() {
        let raw = include_bytes!("../../../docs/rust-rewrite/t07/current-source-bindings.json");
        assert!(validate(raw).is_ok());
        let mut changed = raw.to_vec();
        changed[0] ^= 1;
        assert!(validate(&changed).is_err());
        assert!(validate(b"{}").is_err());
    }
}
