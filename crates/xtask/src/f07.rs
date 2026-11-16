//! Native disposable-store acceptance; no application URL or Python runtime.
use crate::{Result, command, load_json};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    env, fs,
    path::{Path, PathBuf},
};
pub fn check(root: &Path, records: &mut Vec<Value>) -> Result<()> {
    let executable = env::var_os("SLOPSEARX_TEST_VALKEY_SERVER")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("valkey-server"));
    let executable = if executable.components().count() > 1 {
        executable
    } else {
        env::split_paths(&env::var_os("PATH").unwrap_or_default())
            .map(|dir| dir.join(&executable))
            .find(|path| path.is_file())
            .ok_or("disposable Valkey executable missing from PATH")?
    };
    let executable = executable.canonicalize().map_err(|e| e.to_string())?;
    let binary = fs::read(&executable).map_err(|e| e.to_string())?;
    let binary_hash: String = Sha256::digest(&binary)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let version = command(
        root,
        executable.to_str().ok_or("non-UTF8 Valkey executable")?,
        &["--version"],
        records,
    )?;
    if !version.starts_with("Valkey server v=") {
        return Err("test executable is not Valkey".into());
    }
    records.push(json!({"valkey_executable":executable,"sha256":binary_hash,"version":version}));
    let manifest = load_json(&root.join("docs/rust-rewrite/f07/provenance.json"))?;
    if manifest["baseline"] != "7cc420f7ffb61127750e77bf6130e225925f9186" {
        return Err("unreviewed F07 baseline".into());
    }
    let scripts = manifest["scripts"]
        .as_array()
        .ok_or("missing F07 scripts")?;
    if scripts.len() != 24 {
        return Err("incomplete F07 script inventory".into());
    }
    for script in scripts {
        let path = script["path"].as_str().ok_or("missing script path")?;
        let data = fs::read(root.join(path)).map_err(|e| e.to_string())?;
        let hash: String = Sha256::digest(&data)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        if Some(hash.as_str()) != script["sha256"].as_str() {
            return Err(format!("F07 script bytes drifted: {path}"));
        }
        let source = script["source"].as_str().ok_or("missing script source")?;
        let symbol = script["symbol"].as_str().ok_or("missing script symbol")?;
        let pinned = command(
            root,
            "git",
            &[
                "show",
                &format!("7cc420f7ffb61127750e77bf6130e225925f9186:{source}"),
            ],
            records,
        )?;
        let marker = format!("{symbol} = \"\"\"");
        let expected = pinned
            .split_once(&marker)
            .and_then(|(_, tail)| tail.split_once("\"\"\""))
            .map(|(body, _)| body)
            .ok_or("missing pinned Lua symbol")?;
        if expected.as_bytes() != data {
            return Err(format!("F07 Lua differs from pinned source: {path}"));
        }
    }
    command(
        root,
        "cargo",
        &[
            "test",
            "--locked",
            "-p",
            "ssx-infra",
            "--test",
            "valkey",
            "--",
            "--test-threads=1",
        ],
        records,
    )?;
    records.push(json!({"plan_id":"F07","status":"passed","script_count":24,"authority":"private spawned Valkey process and Unix socket; no application store URL","providers":0}));
    Ok(())
}
