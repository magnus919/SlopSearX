//! Native default/installed CLI integration with production HTTP/service/adapter/cache.
//! This qualifies a controlled backend composition, not production startup/cutover.
use crate::{Result, command, t08a};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
const INVENTORY: &[u8] = include_bytes!("../../../docs/rust-rewrite/t08/inventory.json");
const OPTIONAL_WIRE_SHA: &str = "4c45e876fc79dffc0f7da8ccccc9c6ba67d4f4cc9e0bd9e8194f74184b96c65b";
fn need(ok: bool, why: &str) -> Result<()> {
    if ok { Ok(()) } else { Err(why.into()) }
}
fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|v| format!("{v:02x}"))
        .collect()
}
fn image(path: &Path) -> Result<String> {
    let m = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    need(
        m.is_file() && !m.file_type().is_symlink(),
        "T08 image must be a regular file",
    )?;
    Ok(digest(&fs::read(path).map_err(|e| e.to_string())?))
}
fn expected_ids() -> Result<Vec<String>> {
    let inventory: Value = serde_json::from_slice(INVENTORY).map_err(|e| e.to_string())?;
    need(inventory["plan_id"] == "T08", "T08 inventory plan differs")?;
    let ids = inventory["cases"]
        .as_array()
        .ok_or("T08 case inventory missing")?
        .iter()
        .map(|v| {
            v.as_str()
                .map(str::to_owned)
                .ok_or_else(|| "T08 case identity".into())
        })
        .collect::<Result<Vec<_>>>()?;
    need(
        ids.len() == 17 && ids.iter().collect::<BTreeSet<_>>().len() == 17,
        "T08 closed17 case inventory differs",
    )?;
    Ok(ids)
}
fn validate_receipt(
    receipt: &Value,
    ids: &[String],
    cli_sha: &str,
    server_sha: &str,
) -> Result<()> {
    need(
        receipt["cli_sha256"] == cli_sha && receipt["server_sha256"] == server_sha,
        "T08 actual image identity differs",
    )?;
    need(
        receipt["server_and_valkey_waited"] == true
            && receipt["upstream_joined"] == true
            && receipt["private_socket_retired"] == true,
        "T08 actual child/socket/listener retirement missing",
    )?;
    let cases = receipt["records"]
        .as_array()
        .ok_or("T08 actual records missing")?;
    let actual = cases
        .iter()
        .map(|v| v["id"].as_str().ok_or("T08 actual case id missing"))
        .collect::<std::result::Result<Vec<_>, _>>()?;
    need(
        actual == ids.iter().map(String::as_str).collect::<Vec<_>>(),
        "T08 actual case inventory differs",
    )?;
    for case in cases {
        let id = case["id"].as_str().ok_or("T08 case id")?;
        let failed = matches!(
            id,
            "unknown-engine"
                | "negative-page"
                | "upstream-error"
                | "upstream-malformed"
                | "unavailable-server"
        );
        need(
            case["status"] == i32::from(failed)
                && case["binary_sha256"] == cli_sha
                && case["child_waited"] == true,
            "T08 case status/image/wait differs",
        )?;
        let stdout = case["stdout"].as_str().ok_or("T08 stdout bytes missing")?;
        let stderr = case["stderr"].as_str().ok_or("T08 stderr bytes missing")?;
        need(
            if failed {
                stdout.is_empty() && !stderr.is_empty()
            } else {
                !stdout.is_empty() && stderr.is_empty()
            },
            "T08 output/status boundary differs",
        )?;
    }
    for prefix in ["page-0", "page-1"] {
        for repeat in 0..2 {
            let id = format!("{prefix}-{repeat}");
            let record = cases
                .iter()
                .find(|r| r["id"] == id)
                .ok_or("T08 page case absent")?;
            let response: Value =
                serde_json::from_str(record["stdout"].as_str().ok_or("T08 page stdout")?)
                    .map_err(|e| e.to_string())?;
            need(
                response["meta"]["cached"] == (repeat == 1),
                "T08 actual page repeat cache state differs",
            )?;
        }
    }
    let raw = receipt["adapter_calls_raw"]
        .as_str()
        .ok_or("T08 raw adapter calls missing")?;
    let calls = raw
        .lines()
        .map(ssx_json::from_str::<ssx_json::Value>)
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    need(
        calls.len() == 5,
        "T08 actual production adapter call count differs",
    )?;
    let pages = ["18446744073709551616", "18446744073709551617"];
    let page_calls = calls
        .iter()
        .filter(|r| r["query"] == "cache-page")
        .collect::<Vec<_>>();
    need(
        page_calls.len() == 2,
        "T08 distinct cache misses or repeat-hit dispatch differs",
    )?;
    for (call, page) in page_calls.iter().zip(pages) {
        need(
            call["page"].to_string() == page && call["page_text"].as_str() == Some(page),
            "T08 actual lossless adapter page differs",
        )?;
    }
    need(
        calls[0]["query"] == "Unicode café 🦀" && calls[0]["status"] == "ok",
        "T08 Unicode production adapter call differs",
    )?;
    for (query, message) in [
        ("upstream-error", "npm returned an unsuccessful status"),
        ("upstream-malformed", "npm returned invalid JSON"),
    ] {
        let found = calls
            .iter()
            .find(|r| r["query"] == query)
            .ok_or("T08 failed adapter call missing")?;
        need(
            found["status"] == "error" && found["error_message"] == message,
            "T08 native adapter failure classification differs",
        )?;
    }
    let requests = receipt["upstream_requests"]
        .as_array()
        .ok_or("T08 upstream requests missing")?;
    need(
        requests.len() == 5,
        "T08 upstream miss/repeat request count differs",
    )?;
    let mut queries = Vec::new();
    for request in requests {
        let first = request
            .as_str()
            .and_then(|s| s.lines().next())
            .ok_or("T08 upstream request")?;
        let target = first
            .split_whitespace()
            .nth(1)
            .ok_or("T08 upstream target")?;
        let (_, query) = target.split_once('?').ok_or("T08 upstream query")?;
        // The actual driver separately decodes and asserts Unicode query text.
        // Here the two ASCII cache queries can be compared without coercion.
        queries.push(query.to_owned());
    }
    need(
        queries
            .iter()
            .filter(|q| q.contains("text=cache-page"))
            .count()
            == 2,
        "T08 distinct actual upstream page misses differ",
    )?;
    Ok(())
}
fn receipt_controls(
    receipt: &Value,
    ids: &[String],
    cli: &str,
    server: &str,
    records: &mut Vec<Value>,
) -> Result<()> {
    validate_receipt(receipt, ids, cli, server)?;
    let mut omitted = receipt.clone();
    omitted["records"]
        .as_array_mut()
        .ok_or("T08 records")?
        .pop();
    need(
        validate_receipt(&omitted, ids, cli, server)
            .err()
            .as_deref()
            == Some("T08 actual case inventory differs"),
        "T08 omitted actual case accepted",
    )?;
    let mut duplicate = receipt.clone();
    let first = duplicate["records"][0].clone();
    duplicate["records"]
        .as_array_mut()
        .ok_or("T08 records")?
        .push(first);
    need(
        validate_receipt(&duplicate, ids, cli, server)
            .err()
            .as_deref()
            == Some("T08 actual case inventory differs"),
        "T08 duplicate actual case accepted",
    )?;
    let mut wrong = receipt.clone();
    wrong["cli_sha256"] = json!("wrong-image");
    need(
        validate_receipt(&wrong, ids, cli, server).err().as_deref()
            == Some("T08 actual image identity differs"),
        "T08 changed image accepted",
    )?;
    records.push(json!({"T08_positive_actual_receipt_first":true,"counterfactuals":["omit-one-actual-case","duplicate-one-actual-case","change-actual-image"],"scope":"receipt admission controls, not additional runtime cases"}));
    Ok(())
}
fn example_images(root: &Path, records: &mut Vec<Value>) -> Result<(PathBuf, PathBuf)> {
    let raw = command(
        root,
        "cargo",
        &[
            "build",
            "--locked",
            "-p",
            "ssx-http",
            "--example",
            "t08_fixture",
            "--example",
            "t08_cli",
            "--message-format=json",
        ],
        records,
    )?;
    let mut fixture = None;
    let mut driver = None;
    for line in raw.lines() {
        let row: Value = serde_json::from_str(line).map_err(|e| e.to_string())?;
        if row["reason"] != "compiler-artifact"
            || row["manifest_path"].as_str() != root.join("crates/ssx-http/Cargo.toml").to_str()
        {
            continue;
        }
        if row["target"]["kind"] != json!(["example"]) || row["profile"]["test"] != false {
            continue;
        }
        if let Some(path) = row["executable"].as_str() {
            let image_path = PathBuf::from(path);
            let sha = image(&image_path)?;
            match row["target"]["name"].as_str() {
                Some("t08_fixture") => {
                    need(fixture.is_none(), "T08 duplicate server artifact")?;
                    fixture = Some(image_path.clone());
                }
                Some("t08_cli") => {
                    need(driver.is_none(), "T08 duplicate driver artifact")?;
                    driver = Some(image_path.clone());
                }
                _ => continue,
            }
            records.push(json!({"T08_actual_example_compiler_artifact":row,"image_sha256":sha,"image_path":image_path}));
        }
    }
    Ok((
        fixture.ok_or("T08 actual server artifact missing")?,
        driver.ok_or("T08 actual driver artifact missing")?,
    ))
}
struct Images<'a> {
    cli: &'a Path,
    server: &'a Path,
    driver: &'a Path,
}
fn run_driver(
    root: &Path,
    output: &Path,
    images: Images<'_>,
    valkey: &Path,
    temp: &Path,
    ids: &[String],
    records: &mut Vec<Value>,
) -> Result<Value> {
    let Images {
        cli,
        server,
        driver,
    } = images;
    need(!output.exists(), "T08 receipt root must be fresh")?;
    let cli_sha = image(cli)?;
    let server_sha = image(server)?;
    let driver_sha = image(driver)?;
    let valkey_sha = image(valkey)?;
    t08a::bounded(
        root,
        driver,
        &[],
        t08a::NativeChildScope::FullCorpus,
        &[
            ("T08_NATIVE_CLI_BINARY", cli),
            ("T08_NATIVE_SERVER_BINARY", server),
            ("T08_RECEIPTS", output),
            ("SLOPSEARX_TEST_VALKEY_SERVER", valkey),
            ("SLOPSEARX_TEST_TEMP_ROOT", temp),
            ("TMPDIR", temp),
            ("TMP", temp),
            ("TEMP", temp),
        ],
        records,
    )?;
    need(
        image(cli)? == cli_sha
            && image(server)? == server_sha
            && image(driver)? == driver_sha
            && image(valkey)? == valkey_sha,
        "T08 executable changed during actual integration",
    )?;
    let raw = fs::read(output.join("completed.json")).map_err(|e| e.to_string())?;
    let receipt: Value = serde_json::from_slice(&raw).map_err(|e| e.to_string())?;
    need(
        receipt["valkey_sha256"] == valkey_sha,
        "T08 actual private Valkey image differs",
    )?;
    receipt_controls(&receipt, ids, &cli_sha, &server_sha, records)?;
    records.push(json!({"T08_actual_backend_integration":{"receipt":output.join("completed.json"),"receipt_sha256":digest(&raw),"cli_sha256":cli_sha,"server_sha256":server_sha,"driver_sha256":driver_sha,"valkey_sha256":valkey_sha,"cases":ids}}));
    Ok(receipt)
}
pub(crate) fn check(root: &Path, records: &mut Vec<Value>) -> Result<()> {
    need(
        cfg!(unix),
        "T08 backend integration requires a supported native Unix backend host",
    )?;
    need(
        fs::read(root.join("docs/rust-rewrite/t08/inventory.json")).map_err(|e| e.to_string())?
            == INVENTORY,
        "T08 current inventory differs from compiled owner",
    )?;
    let ids = expected_ids()?;
    let host = t08a::native_host(root, records)?;
    let metadata = command(
        root,
        "cargo",
        &["metadata", "--locked", "--no-deps", "--format-version", "1"],
        records,
    )?;
    let metadata: Value = serde_json::from_str(&metadata).map_err(|e| e.to_string())?;
    let target = PathBuf::from(
        metadata["target_directory"]
            .as_str()
            .ok_or("T08 target directory missing")?,
    );
    let valkey = PathBuf::from(
        std::env::var_os("SLOPSEARX_TEST_VALKEY_SERVER")
            .ok_or("T08 requires an explicitly owned private Valkey executable")?,
    );
    if let Ok(expected) = std::env::var("SLOPSEARX_TEST_VALKEY_SERVER_SHA256") {
        need(
            image(&valkey)? == expected,
            "T08 registered private Valkey identity differs",
        )?;
    }
    let temp = PathBuf::from(
        std::env::var_os("SLOPSEARX_TEST_TEMP_ROOT")
            .ok_or("T08 requires owned short socket root")?,
    );
    need(
        valkey.is_absolute() && temp.is_absolute() && temp.is_dir(),
        "T08 requires absolute owned Valkey/temp inputs",
    )?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let output = target.join(format!(
        "rust-evidence/t08-native-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir_all(target.join("rust-evidence")).map_err(|e| e.to_string())?;
    fs::create_dir(&output).map_err(|e| e.to_string())?;
    let (server, driver) = example_images(root, records)?;
    let (cli, default_sha) = t08a::default_shipping_floor(root, records)?;
    t08a::runtime_audit(root, &cli, records)?;
    t08a::runtime_audit(root, &server, records)?;
    let tree = command(
        root,
        "cargo",
        &[
            "tree", "--locked", "-p", "ssx-cli", "--edges", "normal", "--prefix", "none",
        ],
        records,
    )?;
    for forbidden in [
        "ssx-application",
        "ssx-domain",
        "ssx-http",
        "ssx-infra",
        "ssx-server",
        "ssx-worker",
        "ssx-registry",
        "ssx-config",
        "pyo3",
        "cpython",
        "libpython",
        "pypy",
    ] {
        need(
            !tree.lines().any(|line| line.starts_with(forbidden)),
            "T08 native CLI links backend/interpreter implementation",
        )?;
    }
    let default = run_driver(
        root,
        &output.join("default"),
        Images {
            cli: &cli,
            server: &server,
            driver: &driver,
        },
        &valkey,
        &temp,
        &ids,
        records,
    )?;
    let install = output.join("install");
    command(
        root,
        "cargo",
        &[
            "install",
            "--debug",
            "--locked",
            "--path",
            "crates/ssx-cli",
            "--root",
            install.to_str().ok_or("T08 install path UTF8")?,
        ],
        records,
    )?;
    let installed = install.join("bin/ssx");
    t08a::runtime_audit(root, &installed, records)?;
    let installed_receipt = run_driver(
        root,
        &output.join("installed"),
        Images {
            cli: &installed,
            server: &server,
            driver: &driver,
        },
        &valkey,
        &temp,
        &ids,
        records,
    )?;
    let optional = fs::read(root.join("ssx")).map_err(|e| e.to_string())?;
    need(
        digest(&optional) == OPTIONAL_WIRE_SHA,
        "T08 historical optional standalone wire client changed",
    )?;
    let source = command(root, "git", &["rev-parse", "HEAD"], records)?;
    let toolchain = command(root, "rustc", &["--version", "--verbose"], records)?;
    let handoff = json!({"plan":"T08","source_revision":source.trim(),"lock_sha256":image(&root.join("Cargo.lock"))?,"toolchain":toolchain,"native_host":host,"default_cli_sha256":default_sha,"installed_cli_sha256":image(&installed)?,"server_assembly_sha256":image(&server)?,"driver_sha256":image(&driver)?,"valkey_sha256":image(&valkey)?,"default_receipt":output.join("default/completed.json"),"installed_receipt":output.join("installed/completed.json"),"cases":ids,"optional_python_wire_client":{"sha256":OPTIONAL_WIRE_SHA,"disposition":"historical standalone stdlib wire script; no backend imports; neither native runtime invokes or requires it"},"supported_client_source_install_targets":["x86_64-unknown-linux-gnu","aarch64-unknown-linux-gnu","x86_64-apple-darwin","aarch64-apple-darwin","x86_64-pc-windows-msvc"],"installation_guide":"docs/rust-rewrite/t08a/README.md","backend_scope":"real production-component qualification process, not foundation ssx-server or production startup","component_regression":"T08A exact-head CI retains original531 and separate30 native-discovery cases; this gate does not relabel or replace those receipts","downstream":"O04/#921 owns release archives/signing/promotion and production install/upgrade/uninstall","production_activation":false});
    fs::write(
        output.join("O04-HANDOFF.json"),
        serde_json::to_vec_pretty(&handoff).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    records.push(json!({"T08_O04_handoff":handoff,"default_actual_receipt":default,"installed_actual_receipt":installed_receipt}));
    Ok(())
}
