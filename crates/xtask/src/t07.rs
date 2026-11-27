//! Source-bound native gateway parity and lifecycle qualification.
use crate::{Result, command};
use serde_json::{Value, json};
use std::{collections::BTreeSet, path::Path};

const PARITY: &[&str] = &["direct_and_gateway_clients_preserve_search_surface_contracts"];
const STARTUP: &[&str] = &[
    "cancelled_gateway_startup_retires_the_granted_session",
    "cancelled_gateway_stdio_retires_session_with_pending_rpc_and_open_input",
];

fn run_suite(root: &Path, target: &str, expected: &[&str], records: &mut Vec<Value>) -> Result<()> {
    let args = vec!["test", "--locked", "-p", "ssx-mcp", "--test", target];
    let mut listing = args.clone();
    listing.extend_from_slice(&["--", "--list"]);
    let output = command(root, "cargo", &listing, records)?;
    let actual: BTreeSet<_> = output
        .lines()
        .filter_map(|line| line.strip_suffix(": test"))
        .collect();
    let required: BTreeSet<_> = expected.iter().copied().collect();
    if actual != required || actual.len() != expected.len() {
        return Err(format!("T07 {target} test inventory differs: {actual:?}"));
    }
    let output = command(root, "cargo", &args, records)?;
    let result = format!(
        "test result: ok. {} passed; 0 failed; 0 ignored;",
        expected.len()
    );
    if !output.contains(&result) {
        return Err(format!("T07 empty/failed/ignored {target} tests"));
    }
    Ok(())
}

pub fn check(root: &Path, records: &mut Vec<Value>) -> Result<()> {
    run_suite(root, "t07_gateway_parity", PARITY, records)?;
    run_suite(root, "gateway_startup", STARTUP, records)?;
    records.push(json!({
        "plan_id": "T07",
        "status": "passed",
        "scope": "exact native direct/gateway search-surface parity, structured and protocol errors, and cancellable gateway startup/session retirement; installed public-root HTTPS is a separate required Apple Silicon hosted check"
    }));
    Ok(())
}
