//! O03 inventory and prospective contract validation. This check performs no experiment work.
mod numeric;
use crate::{Result, command};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, fs, path::Path};

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Artifact {
    sha256: String,
    size_bytes: u64,
    path: String,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceFile {
    path: String,
    git_blob: String,
    sha256: String,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceBinding {
    git_commit: String,
    dirty: bool,
    tree_sha256: String,
    files: Vec<SourceFile>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Toolchain {
    rustc: String,
    cargo: String,
    target: String,
    lockfile_sha256: String,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceReceipt {
    schema_version: String,
    receipt_kind: String,
    plan_id: String,
    git_commit: String,
    dirty: bool,
    tree_sha256: String,
    files: Vec<SourceFile>,
    binary: Artifact,
    toolchain: Toolchain,
    configuration_sha256: String,
}

#[derive(serde::Deserialize)]
#[serde(tag = "status", deny_unknown_fields)]
enum ActualCost {
    #[serde(rename = "known")]
    Known { microusd: u64 },
    #[serde(rename = "unknown")]
    Unknown { reason_code: String },
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RuntimeAccounting {
    requests_reserved: u64,
    requests_dispatched: u64,
    requests_settled: u64,
    requests_unresolved: u64,
    requests_released: u64,
    cost_microusd_reserved: u64,
    actual_cost: ActualCost,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Authority {
    issuer: String,
    reservation_id: String,
    authorization_sha256: String,
    scope: Vec<String>,
    status: String,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Clock {
    source: String,
    started_unix_ms: u64,
    finished_unix_ms: u64,
    monotonic_elapsed_ms: u64,
    deadline_unix_ms: u64,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Reservation {
    max_requests: u64,
    max_cost_microusd: u64,
    deadline_unix_ms: u64,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RuntimeConfiguration {
    sha256: String,
    redacted: bool,
    effective_keys: Vec<String>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RuntimeReceipt {
    schema_version: String,
    receipt_kind: String,
    run_id: String,
    plan_id: String,
    binary: Artifact,
    source: SourceBinding,
    toolchain: Toolchain,
    configuration: RuntimeConfiguration,
    authority: Authority,
    clock: Clock,
    reservation: Reservation,
    accounting: RuntimeAccounting,
    outcome: String,
    failure: Option<RuntimeFailure>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RuntimeFailure {
    reason_code: String,
    terminal_evidence: Artifact,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct QualificationCheck {
    check_id: String,
    status: String,
    evidence_sha256: String,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct QualificationReceipt {
    schema_version: String,
    plan_id: String,
    runtime_receipt_sha256: String,
    checks: Vec<QualificationCheck>,
    outcome: String,
    reason_codes: Vec<String>,
    source_sha256: String,
    decision_timestamp_unix_ms: u64,
}

fn ensure(ok: bool, why: &str) -> Result<()> {
    if ok { Ok(()) } else { Err(why.into()) }
}

fn validate_inventory(value: &Value, active_paths: &BTreeSet<String>) -> Result<()> {
    ensure(
        value["schema_version"].as_str() == Some("1.0.0"),
        "O03 inventory schema version must be 1.0.0",
    )?;
    ensure(
        value["inventory_kind"].as_str() == Some("active_scripts_directory_exact_set"),
        "O03 inventory kind is not exact-set",
    )?;
    let rows = value["scripts"]
        .as_array()
        .ok_or("O03 scripts must be an array")?;
    ensure(!rows.is_empty(), "O03 inventory cannot be empty")?;
    let mut seen = BTreeSet::new();
    for row in rows {
        let path = row["path"].as_str().ok_or("O03 row missing path")?;
        ensure(
            path.starts_with("scripts/") && path.ends_with(".py"),
            "O03 path outside active scripts directory",
        )?;
        ensure(seen.insert(path.to_string()), "duplicate O03 script path")?;
        let owner = row["owner"].as_str().ok_or("O03 row missing owner")?;
        ensure(
            matches!(owner, "O02" | "O07" | "O08"),
            "O03 script has invalid migration owner",
        )?;
        for key in [
            "source_blob",
            "current_entrypoint",
            "entrypoint_kind",
            "entrypoint_functions",
            "api_signatures",
            "cli_options",
            "source_summary",
            "inputs",
            "outputs",
            "internal_imports",
            "direct_backend_imports",
            "consumed_by",
            "native_successor",
            "boundary",
            "disposition",
            "stage",
            "handoff_validation",
        ] {
            ensure(!row[key].is_null(), &format!("O03 {path} missing {key}"))?;
        }
        for key in [
            "source_summary",
            "current_entrypoint",
            "native_successor",
            "boundary",
            "handoff_validation",
        ] {
            ensure(
                row[key].as_str().is_some_and(|s| !s.trim().is_empty()),
                &format!("O03 {path} has empty {key}"),
            )?;
        }
        ensure(
            row["source_blob"]
                .as_str()
                .is_some_and(|s| s.len() == 40 && s.bytes().all(|b| b.is_ascii_hexdigit())),
            &format!("O03 {path} has invalid source blob"),
        )?;
        ensure(
            row["inputs"].as_array().is_some_and(|a| !a.is_empty()),
            &format!("O03 {path} has no declared inputs"),
        )?;
        ensure(
            row["outputs"].as_array().is_some_and(|a| !a.is_empty()),
            &format!("O03 {path} has no declared outputs"),
        )?;
        let native = row["native_successor"].as_str().unwrap_or_default();
        let owner_command = match owner {
            "O07" => native.starts_with("ssx experiment "),
            "O08" => native.starts_with("ssx evidence "),
            "O02" => native.starts_with("excluded O02 "),
            _ => false,
        };
        ensure(
            owner_command,
            &format!("O03 {path} has no owner-scoped native command mapping"),
        )?;
        ensure(
            row["boundary"]
                .as_str()
                .is_some_and(|s| s.contains("no Python backend imports")),
            &format!("O03 {path} boundary does not exclude backend imports"),
        )?;
        ensure(
            row["handoff_validation"]
                .as_str()
                .is_some_and(|s| s.contains("offline fixtures")),
            &format!("O03 {path} lacks offline handoff validation"),
        )?;
        ensure(
            matches!(
                row["entrypoint_kind"].as_str(),
                Some("cli" | "programmatic_module")
            ) && row["entrypoint_functions"].is_array()
                && row["api_signatures"].is_array()
                && row["cli_options"].is_array()
                && row["internal_imports"].is_array()
                && row["direct_backend_imports"].is_array()
                && row["consumed_by"].is_array(),
            &format!("O03 {path} has malformed entrypoint or graph fields"),
        )?;
    }
    ensure(
        seen == *active_paths,
        "O03 inventory does not exactly cover scripts/*.py",
    )?;
    ensure(
        rows.iter().any(|r| r["owner"] == "O07") && rows.iter().any(|r| r["owner"] == "O08"),
        "O03 inventory omits O07 or O08 ownership",
    )?;
    Ok(())
}

fn check_source_hashes(root: &Path, value: &Value, records: &mut Vec<Value>) -> Result<()> {
    for row in value["scripts"]
        .as_array()
        .ok_or("O03 scripts must be an array")?
    {
        let path = row["path"].as_str().ok_or("O03 row missing path")?;
        let actual = command(root, "git", &["hash-object", "--", path], records)?;
        let expected = row["source_blob"].as_str().unwrap_or_default();
        ensure(
            source_hash_matches(root, path, expected, actual.trim())?,
            &format!("O03 source drift: {path}; refresh reviewed inventory"),
        )?;
    }
    Ok(())
}

fn source_hash_matches(root: &Path, path: &str, expected: &str, actual: &str) -> Result<bool> {
    if actual == expected {
        return Ok(true);
    }
    crate::o02_health_retirement::successor(root, path, expected)
}

fn check_supporting_paths(root: &Path, value: &Value, records: &mut Vec<Value>) -> Result<()> {
    let expected: BTreeSet<&str> = BTreeSet::from([
        "tests/test_coverage_saved_method_materials.py",
        "tests/test_intent_ranking_replay.py",
        "docs/experiments/evidence/coverage-first-study/w0-rerank-v1.py.txt",
    ]);
    let rows = value["supporting_paths"]
        .as_array()
        .ok_or("O03 supporting_paths must be an array")?;
    let mut seen = BTreeSet::new();
    for row in rows {
        let path = row["path"].as_str().ok_or("O03 support path missing")?;
        ensure(seen.insert(path), "duplicate O03 supporting path")?;
        ensure(
            row["role"].as_str().is_some_and(|s| !s.trim().is_empty())
                && row["current_command_or_consumer"]
                    .as_str()
                    .is_some_and(|s| !s.trim().is_empty()),
            &format!("O03 supporting path {path} lacks cold-agent role/command"),
        )?;
        let actual = command(root, "git", &["hash-object", "--", path], records)?;
        ensure(
            actual.trim() == row["source_blob"].as_str().unwrap_or_default(),
            &format!("O03 supporting source drift: {path}; refresh reviewed inventory"),
        )?;
    }
    ensure(
        seen == expected,
        "O03 regression/dynamic-source anchors are incomplete",
    )?;
    let surfaces = &value["execution_surfaces"];
    ensure(
        surfaces["mandatory_ci"].as_array().is_some_and(|gates| {
            ["F01", "F02", "F03", "O06", "O03"].iter().all(|id| {
                gates
                    .iter()
                    .any(|g| g.as_str().is_some_and(|s| s.contains(id)))
            })
        }),
        "O03 execution surface omits a mandatory inventory/CI gate",
    )?;
    ensure(
        surfaces["ci_python_invocation_scope"]
            .as_str()
            .is_some_and(|s| {
                s.contains("63 interpreter invocations")
                    && s.contains("106 interpreter configurations")
                    && s.contains("49 documented evidence examples")
            }),
        "O03 execution surface does not account for current F01 invocation classes",
    )?;
    records.push(
        serde_json::json!({"check":"o03_supporting_paths","anchors":rows.len(),"status":"passed"}),
    );
    Ok(())
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn validate_artifact(a: &Artifact) -> Result<()> {
    ensure(valid_sha256(&a.sha256), "artifact SHA-256 malformed")?;
    ensure(a.size_bytes > 0, "artifact size must be positive")?;
    ensure(!a.path.trim().is_empty(), "artifact path is empty")
}

// Source receipt references are portable repository paths, not arbitrary host
// filenames. Forward slashes are canonical; reject Windows drive/separator and
// trailing-dot/space aliases even when validating on Unix.
fn portable_source_path(path: &str) -> bool {
    !path.trim().is_empty()
        && !path.contains(['\\', ':', '\0'])
        && path.split('/').all(|part| {
            !part.is_empty() && part != "." && part != ".." && !part.ends_with(['.', ' '])
        })
}

fn validate_source_files(files: &[SourceFile]) -> Result<()> {
    ensure(!files.is_empty(), "source receipt has no source files")?;
    let mut paths = BTreeSet::new();
    for file in files {
        ensure(
            portable_source_path(&file.path),
            "source path must be a portable canonical repository-relative path",
        )?;
        ensure(paths.insert(file.path.as_str()), "duplicate source path")?;
        ensure(
            file.git_blob.len() == 40
                && file
                    .git_blob
                    .bytes()
                    .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()),
            "source Git blob malformed",
        )?;
        ensure(valid_sha256(&file.sha256), "source file SHA-256 malformed")?;
    }
    Ok(())
}

fn validate_toolchain(t: &Toolchain) -> Result<()> {
    ensure(
        !t.rustc.trim().is_empty() && !t.cargo.trim().is_empty() && !t.target.trim().is_empty(),
        "toolchain identity is incomplete",
    )?;
    ensure(
        valid_sha256(&t.lockfile_sha256),
        "lockfile SHA-256 malformed",
    )
}

fn validate_source_receipt(value: Value) -> Result<()> {
    let receipt: SourceReceipt =
        serde_json::from_value(value).map_err(|e| format!("source receipt shape: {e}"))?;
    ensure(
        receipt.schema_version == "1.0.0" && receipt.receipt_kind == "native_source",
        "source receipt version or kind invalid",
    )?;
    ensure(
        valid_plan_id(&receipt.plan_id),
        "source receipt plan id violates schema pattern",
    )?;
    ensure(
        valid_git_commit(&receipt.git_commit),
        "source Git commit malformed",
    )?;
    ensure(!receipt.dirty, "source receipt must bind a clean tree")?;
    ensure(
        valid_sha256(&receipt.tree_sha256) && valid_sha256(&receipt.configuration_sha256),
        "source or configuration digest malformed",
    )?;
    validate_source_files(&receipt.files)?;
    validate_artifact(&receipt.binary)?;
    validate_toolchain(&receipt.toolchain)
}

fn validate_source_binding(source: &SourceBinding) -> Result<()> {
    ensure(
        valid_git_commit(&source.git_commit),
        "runtime source Git commit malformed",
    )?;
    ensure(
        !source.dirty,
        "runtime receipt must bind a clean source tree",
    )?;
    ensure(
        valid_sha256(&source.tree_sha256),
        "runtime source tree digest malformed",
    )?;
    validate_source_files(&source.files)
}

fn valid_plan_id(value: &str) -> bool {
    let mut bytes = value.bytes();
    bytes.next().is_some_and(|b| b.is_ascii_uppercase())
        && bytes.all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'-')
}

fn valid_git_commit(value: &str) -> bool {
    matches!(value.len(), 40 | 64)
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn validate_runtime_receipt(value: Value) -> Result<()> {
    let r: RuntimeReceipt =
        serde_json::from_value(value).map_err(|e| format!("runtime receipt shape: {e}"))?;
    ensure(
        r.schema_version == "1.0.0" && r.receipt_kind == "native_runtime",
        "runtime receipt version or kind invalid",
    )?;
    ensure(
        !r.run_id.trim().is_empty() && valid_plan_id(&r.plan_id),
        "runtime receipt identity missing",
    )?;
    validate_artifact(&r.binary)?;
    validate_source_binding(&r.source)?;
    validate_toolchain(&r.toolchain)?;
    ensure(
        valid_sha256(&r.configuration.sha256) && r.configuration.redacted,
        "runtime configuration must be digest-bound and redacted",
    )?;
    ensure(
        !r.configuration.effective_keys.is_empty()
            && r.configuration
                .effective_keys
                .iter()
                .all(|key| !key.trim().is_empty())
            && r.configuration
                .effective_keys
                .iter()
                .collect::<BTreeSet<_>>()
                .len()
                == r.configuration.effective_keys.len(),
        "runtime configuration requires unique, nonempty effective key names",
    )?;
    ensure(
        !r.authority.issuer.trim().is_empty() && !r.authority.reservation_id.trim().is_empty(),
        "authority identity missing",
    )?;
    ensure(
        valid_sha256(&r.authority.authorization_sha256),
        "authorization digest malformed",
    )?;
    ensure(
        !r.authority.scope.is_empty() && r.authority.scope.iter().all(|s| !s.trim().is_empty()),
        "authority scope missing",
    )?;
    ensure(
        r.authority.scope.iter().collect::<BTreeSet<_>>().len() == r.authority.scope.len(),
        "authority scope contains duplicates",
    )?;
    ensure(
        matches!(r.authority.status.as_str(), "authorized" | "denied"),
        "authority status invalid",
    )?;
    ensure(
        matches!(r.clock.source.as_str(), "fixed" | "system"),
        "clock source invalid",
    )?;
    ensure(
        r.clock.finished_unix_ms >= r.clock.started_unix_ms,
        "wall clock interval is reversed",
    )?;
    ensure(
        (r.accounting.requests_dispatched == 0
            && r.authority.status == "denied"
            && r.outcome == "blocked")
            || r.clock.monotonic_elapsed_ms
                <= r.clock
                    .deadline_unix_ms
                    .saturating_sub(r.clock.started_unix_ms),
        "monotonic duration exceeds deadline budget",
    )?;
    ensure(
        r.reservation.deadline_unix_ms == r.clock.deadline_unix_ms,
        "reservation and clock deadlines differ",
    )?;
    ensure(
        r.accounting.requests_dispatched == 0
            || r.clock.started_unix_ms <= r.reservation.deadline_unix_ms,
        "dispatch began after its reservation deadline",
    )?;
    ensure(
        r.accounting.requests_reserved <= r.reservation.max_requests,
        "requests reserved exceed authorized cap",
    )?;
    ensure(
        r.accounting.requests_dispatched <= r.accounting.requests_reserved,
        "dispatches exceed reserved requests",
    )?;
    ensure(
        r.accounting.requests_reserved
            == r.accounting
                .requests_dispatched
                .checked_add(r.accounting.requests_released)
                .ok_or("request accounting overflow")?,
        "reserved requests do not equal dispatched plus released",
    )?;
    ensure(
        r.accounting.requests_dispatched
            == r.accounting
                .requests_settled
                .checked_add(r.accounting.requests_unresolved)
                .ok_or("request settlement overflow")?,
        "dispatched requests do not equal settled plus unresolved",
    )?;
    ensure(
        r.accounting.cost_microusd_reserved <= r.reservation.max_cost_microusd,
        "reserved cost exceeds authorized cap",
    )?;
    let known_cost = match &r.accounting.actual_cost {
        ActualCost::Known { microusd } => Some(*microusd),
        ActualCost::Unknown { reason_code } => {
            ensure(
                !reason_code.trim().is_empty(),
                "unknown actual cost requires a reason code",
            )?;
            None
        }
    };
    if let Some(actual) = known_cost {
        ensure(
            actual <= r.accounting.cost_microusd_reserved,
            "actual cost exceeds reserved cost",
        )?;
    }
    if r.authority.status == "denied" {
        ensure(
            r.accounting.requests_dispatched == 0,
            "denied authority cannot have dispatched requests",
        )?;
    }
    ensure(
        matches!(
            r.outcome.as_str(),
            "accepted" | "rejected" | "inconclusive" | "blocked" | "failure" | "outcome_unknown"
        ),
        "runtime outcome invalid",
    )?;
    match (r.outcome.as_str(), r.failure.as_ref()) {
        ("failure", Some(failure)) => {
            ensure(
                !failure.reason_code.trim().is_empty(),
                "failure outcome requires a reason code",
            )?;
            validate_artifact(&failure.terminal_evidence)?;
        }
        ("failure", None) => {
            return Err("failure outcome requires reason and terminal evidence".into());
        }
        (_, Some(_)) => {
            return Err("failure details are only valid for failure outcomes".into());
        }
        (_, None) => {}
    }
    ensure(
        r.accounting.requests_unresolved == 0 || r.outcome == "outcome_unknown",
        "unresolved dispatched work requires outcome_unknown, never failure or acceptance",
    )?;
    if r.outcome == "accepted" {
        ensure(
            r.authority.status == "authorized",
            "accepted outcome requires authorized authority",
        )?;
        ensure(
            r.accounting.requests_unresolved == 0,
            "accepted outcome cannot have unresolved dispatches",
        )?;
        ensure(
            known_cost.is_some(),
            "accepted outcome requires known actual cost; unknown is not zero",
        )?;
        ensure(
            r.clock.finished_unix_ms <= r.clock.deadline_unix_ms,
            "accepted outcome completed after its deadline",
        )?;
    }
    if r.outcome == "outcome_unknown" {
        ensure(
            r.accounting.requests_unresolved > 0 || known_cost.is_none(),
            "outcome_unknown requires unresolved work or unknown cost evidence",
        )?;
    }
    Ok(())
}

fn validate_qualification_receipt(value: Value) -> Result<()> {
    let q: QualificationReceipt =
        serde_json::from_value(value).map_err(|e| format!("qualification receipt shape: {e}"))?;
    ensure(
        q.schema_version == "1.0.0" && valid_plan_id(&q.plan_id),
        "qualification identity/version invalid",
    )?;
    ensure(
        valid_plan_id(&q.plan_id),
        "qualification plan ID violates schema pattern",
    )?;
    ensure(
        valid_sha256(&q.runtime_receipt_sha256) && valid_sha256(&q.source_sha256),
        "qualification bindings malformed",
    )?;
    ensure(
        q.decision_timestamp_unix_ms > 0 && !q.checks.is_empty(),
        "qualification requires checks and a timestamp",
    )?;
    let mut checks = BTreeSet::new();
    for check in &q.checks {
        ensure(
            !check.check_id.trim().is_empty() && checks.insert(check.check_id.as_str()),
            "qualification check ID is empty or duplicated",
        )?;
        ensure(
            matches!(
                check.status.as_str(),
                "passed" | "failed" | "not_run" | "unknown"
            ),
            "qualification check status invalid",
        )?;
        ensure(
            valid_sha256(&check.evidence_sha256),
            "qualification evidence digest malformed",
        )?;
    }
    ensure(
        matches!(
            q.outcome.as_str(),
            "accepted" | "rejected" | "inconclusive" | "blocked" | "failure" | "outcome_unknown"
        ),
        "qualification outcome invalid",
    )?;
    if q.outcome == "accepted" {
        ensure(
            q.checks.iter().all(|c| c.status == "passed"),
            "accepted qualification requires every check to pass",
        )?;
        ensure(
            q.reason_codes.is_empty(),
            "accepted qualification cannot carry rejection reason codes",
        )?;
    } else {
        ensure(
            !q.reason_codes.is_empty(),
            "nonaccepted qualification requires explicit reason codes",
        )?;
    }
    ensure(
        q.reason_codes.iter().all(|s| !s.trim().is_empty()),
        "qualification reason code is empty",
    )?;
    ensure(
        q.reason_codes.iter().collect::<BTreeSet<_>>().len() == q.reason_codes.len(),
        "qualification reason codes contain duplicates",
    )
}

fn validate_qualification_fixture(value: Value, root: &Path) -> Result<()> {
    validate_qualification_receipt(value.clone())?;
    let q: QualificationReceipt =
        serde_json::from_value(value).map_err(|e| format!("qualification receipt shape: {e}"))?;
    let runtime_path = root.join("crates/xtask/fixtures/o03/runtime-receipt.accepted.json");
    let runtime_bytes = fs::read(&runtime_path).map_err(|e| e.to_string())?;
    let runtime = numeric::receipt_value(&runtime_bytes)?;
    ensure(
        q.plan_id == runtime["plan_id"].as_str().unwrap_or_default(),
        "qualification plan does not match its runtime receipt",
    )?;
    ensure(
        q.runtime_receipt_sha256 == sha256_hex(&runtime_bytes),
        "qualification does not bind the exact runtime receipt bytes",
    )?;
    ensure(
        q.source_sha256
            == runtime["source"]["tree_sha256"]
                .as_str()
                .unwrap_or_default(),
        "qualification source digest does not match its runtime receipt",
    )?;
    ensure(
        q.decision_timestamp_unix_ms
            >= runtime["clock"]["finished_unix_ms"]
                .as_u64()
                .unwrap_or(u64::MAX),
        "qualification decision predates runtime completion",
    )?;
    if q.outcome == "accepted" {
        ensure(
            runtime["outcome"] == "accepted",
            "accepted qualification requires an accepted runtime receipt",
        )?;
    }
    Ok(())
}

fn validate_contract_fixtures(root: &Path, records: &mut Vec<Value>) -> Result<()> {
    let cases = [
        ("source-receipt.valid.json", "source", true, true),
        ("source-receipt.invalid.json", "source", false, false),
        ("runtime-receipt.accepted.json", "runtime", true, true),
        (
            "runtime-receipt.outcome-unknown.json",
            "runtime",
            true,
            true,
        ),
        (
            "runtime-receipt.invalid-failure-reason.json",
            "runtime",
            false,
            false,
        ),
        (
            "runtime-receipt.invalid-failure-terminal-evidence.json",
            "runtime",
            false,
            false,
        ),
        (
            "runtime-receipt.invalid-failure-on-accepted.json",
            "runtime",
            false,
            false,
        ),
        ("runtime-receipt.failure.json", "runtime", true, true),
        (
            "runtime-receipt.invalid-cost-accepted.json",
            "runtime",
            false,
            false,
        ),
        (
            "runtime-receipt.invalid-conservation.json",
            "runtime",
            false,
            true,
        ),
        (
            "runtime-receipt.invalid-request-cap.json",
            "runtime",
            false,
            true,
        ),
        (
            "runtime-receipt.invalid-cost-cap.json",
            "runtime",
            false,
            true,
        ),
        (
            "runtime-receipt.invalid-deadline.json",
            "runtime",
            false,
            true,
        ),
        (
            "runtime-receipt.invalid-deadline-binding.json",
            "runtime",
            false,
            true,
        ),
        (
            "runtime-receipt.invalid-authority.json",
            "runtime",
            false,
            false,
        ),
        ("runtime-receipt.invalid-plan.json", "runtime", false, false),
        (
            "runtime-receipt.invalid-scope-duplicate.json",
            "runtime",
            false,
            false,
        ),
        (
            "runtime-receipt.invalid-unresolved-failure.json",
            "runtime",
            false,
            true,
        ),
        (
            "runtime-receipt.invalid-expired-dispatch.json",
            "runtime",
            false,
            true,
        ),
        (
            "qualification-result.valid.json",
            "qualification",
            true,
            true,
        ),
        (
            "qualification-result.invalid.json",
            "qualification",
            false,
            true,
        ),
        (
            "qualification-result.invalid-binding.json",
            "qualification",
            false,
            true,
        ),
        (
            "qualification-result.invalid-duplicate-reasons.json",
            "qualification",
            false,
            false,
        ),
        (
            "qualification-result.invalid-stale-decision.json",
            "qualification",
            false,
            true,
        ),
    ];
    let mut schema_cache = std::collections::BTreeMap::new();
    for (name, kind, expected_valid, expected_schema_valid) in cases {
        let path = root.join("crates/xtask/fixtures/o03").join(name);
        let value = numeric::receipt_value(
            &fs::read(&path).map_err(|e| format!("missing O03 fixture {}: {e}", path.display()))?,
        )
        .map_err(|e| format!("invalid O03 fixture {}: {e}", path.display()))?;
        let schema_name = match kind {
            "source" => "source-receipt.schema.json",
            "runtime" => "runtime-receipt.schema.json",
            "qualification" => "qualification-result.schema.json",
            _ => "unknown.schema.json",
        };
        let schema = if let Some(schema) = schema_cache.get(schema_name) {
            schema
        } else {
            let schema_path = root.join("contracts/o03").join(schema_name);
            let schema: Value = serde_json::from_slice(
                &fs::read(&schema_path)
                    .map_err(|e| format!("missing schema {}: {e}", schema_path.display()))?,
            )
            .map_err(|e| format!("invalid schema {}: {e}", schema_path.display()))?;
            schema_cache.insert(schema_name, schema);
            schema_cache.get(schema_name).expect("just inserted")
        };
        let schema_result = validate_schema_instance(schema, &value, schema, name);
        let schema_valid = schema_result.is_ok();
        ensure(
            schema_valid == expected_schema_valid,
            &format!(
                "O03 fixture {name} schema result unexpected: {}",
                schema_result.err().unwrap_or_default()
            ),
        )?;
        let semantic_result = match kind {
            "source" => validate_source_receipt(value),
            "runtime" => validate_runtime_receipt(value),
            "qualification" => validate_qualification_fixture(value, root),
            _ => Err("unknown O03 fixture kind".into()),
        };
        let result_valid = schema_valid && semantic_result.is_ok();
        ensure(
            result_valid == expected_valid,
            &format!(
                "O03 fixture {name} validation unexpected: {}",
                semantic_result.err().unwrap_or_default()
            ),
        )?;
    }
    records.push(serde_json::json!({"check":"o03_receipt_fixtures","cases":cases.len(),"valid_examples":5,"negative_examples":cases.len()-5,"schema_valid_negative_examples":10,"status":"passed"}));
    Ok(())
}

fn validate_schema_tree(schema: &Value, root: &Value, where_: &str) -> Result<()> {
    if schema.is_boolean() {
        return Ok(());
    }
    let object = schema
        .as_object()
        .ok_or_else(|| format!("{where_}: schema node must be an object or boolean"))?;
    const SCHEMA_KEYWORDS: &[&str] = &[
        "$schema",
        "$id",
        "$defs",
        "$ref",
        "title",
        "description",
        "type",
        "additionalProperties",
        "required",
        "properties",
        "items",
        "minItems",
        "uniqueItems",
        "minLength",
        "pattern",
        "minimum",
        "const",
        "enum",
        "oneOf",
        "allOf",
        "if",
        "then",
    ];
    for key in object.keys() {
        ensure(
            SCHEMA_KEYWORDS.contains(&key.as_str()),
            &format!("{where_}: unsupported schema keyword {key}"),
        )?;
    }
    if let Some(pattern) = object.get("pattern").and_then(Value::as_str) {
        ensure(
            matches!(
                pattern,
                "^[A-Z][A-Z0-9-]*$"
                    | "^[a-f0-9]{64}$"
                    | "^[a-f0-9]{40}$"
                    | "^([a-f0-9]{40}|[a-f0-9]{64})$"
            ),
            &format!("{where_}: unsupported pattern {pattern}"),
        )?;
    }
    if object.contains_key("pattern") {
        ensure(
            object.get("pattern").and_then(Value::as_str).is_some(),
            &format!("{where_}: pattern must be a string"),
        )?;
    }
    if object.contains_key("type") {
        let kind = object
            .get("type")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("{where_}: type must be a supported string"))?;
        ensure(
            matches!(
                kind,
                "object" | "array" | "string" | "boolean" | "integer" | "number"
            ),
            &format!("{where_}: unsupported JSON type {kind}"),
        )?;
    }
    if let Some(properties) = object.get("properties") {
        ensure(
            properties.is_object(),
            &format!("{where_}: properties must be an object"),
        )?;
    }
    if let Some(policy) = object.get("additionalProperties") {
        ensure(
            policy.is_boolean() || policy.is_object(),
            &format!("{where_}: additionalProperties must be boolean or a schema"),
        )?;
    }
    if let Some(items) = object.get("items") {
        ensure(
            items.is_object() || items.is_boolean(),
            &format!("{where_}: items must be a schema"),
        )?;
    }
    for key in ["if", "then"] {
        if let Some(branch) = object.get(key) {
            ensure(
                branch.is_object() || branch.is_boolean(),
                &format!("{where_}: {key} must be a schema"),
            )?;
        }
    }
    if object.contains_key("required") {
        ensure(
            object.get("required").and_then(Value::as_array).is_some(),
            &format!("{where_}: required must be an array"),
        )?;
    }
    if let Some(required) = object.get("required").and_then(Value::as_array) {
        let names: Vec<_> = required.iter().filter_map(Value::as_str).collect();
        ensure(
            names.len() == required.len()
                && names.iter().collect::<BTreeSet<_>>().len() == names.len(),
            &format!("{where_}: required fields must be unique strings"),
        )?;
    }
    for key in ["minItems", "minLength", "minimum"] {
        if let Some(bound) = object.get(key) {
            ensure(
                bound.as_u64().is_some()
                    || (key == "minimum" && bound.as_f64().is_some_and(|number| number >= 0.0)),
                &format!("{where_}: {key} must be a nonnegative number"),
            )?;
        }
    }
    if let Some(unique) = object.get("uniqueItems") {
        ensure(
            unique.is_boolean(),
            &format!("{where_}: uniqueItems must be boolean"),
        )?;
    }
    if let Some(values) = object.get("enum") {
        ensure(
            values.as_array().is_some_and(|values| !values.is_empty()),
            &format!("{where_}: enum must be a nonempty array"),
        )?;
        if let Some(values) = values.as_array() {
            ensure(
                (0..values.len()).all(|index| !values[index + 1..].contains(&values[index])),
                &format!("{where_}: enum values must be unique"),
            )?;
        }
    }
    for key in ["oneOf", "allOf"] {
        if let Some(branches) = object.get(key) {
            ensure(
                branches
                    .as_array()
                    .is_some_and(|branches| !branches.is_empty()),
                &format!("{where_}: {key} must be a nonempty array"),
            )?;
        }
    }
    if object.get("type").and_then(Value::as_str) == Some("object") {
        let properties = object
            .get("properties")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        let required = object
            .get("required")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        for field in required {
            let key = field
                .as_str()
                .ok_or("schema required entry must be a string")?;
            ensure(
                properties.contains_key(key),
                &format!("{where_}: required field {key} has no property schema"),
            )?;
        }
        ensure(
            object.contains_key("additionalProperties"),
            &format!("{where_}: object lacks an unknown-field policy"),
        )?;
    }
    if object.contains_key("$ref") {
        ensure(
            object.get("$ref").and_then(Value::as_str).is_some(),
            &format!("{where_}: $ref must be a string"),
        )?;
    }
    if let Some(reference) = object.get("$ref").and_then(Value::as_str) {
        ensure(
            reference.starts_with("#/$defs/"),
            &format!("{where_}: unsupported nonlocal reference {reference}"),
        )?;
        let name = reference.trim_start_matches("#/$defs/");
        ensure(
            root["$defs"].get(name).is_some(),
            &format!("{where_}: unresolved reference {reference}"),
        )?;
    }
    // Only schema-valued keywords recurse. `properties` and `$defs` are
    // maps whose keys are names, not keywords; check their values as nodes.
    for key in ["properties", "$defs"] {
        if let Some(children) = object.get(key).and_then(Value::as_object) {
            for (name, child) in children {
                validate_schema_tree(child, root, &format!("{where_}/{key}/{name}"))?;
            }
        }
    }
    for key in ["items", "additionalProperties", "if", "then"] {
        if let Some(child) = object.get(key)
            && (child.is_object() || child.is_boolean())
        {
            validate_schema_tree(child, root, &format!("{where_}/{key}"))?;
        }
    }
    for key in ["oneOf", "allOf"] {
        if let Some(children) = object.get(key).and_then(Value::as_array) {
            for (index, child) in children.iter().enumerate() {
                validate_schema_tree(child, root, &format!("{where_}/{key}/{index}"))?;
            }
        }
    }
    Ok(())
}

fn schema_pattern_matches(pattern: &str, value: &str) -> bool {
    match pattern {
        "^[A-Z][A-Z0-9-]*$" => valid_plan_id(value),
        "^[a-f0-9]{64}$" => valid_sha256(value),
        "^[a-f0-9]{40}$" => {
            value.len() == 40
                && value
                    .bytes()
                    .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        }
        "^([a-f0-9]{40}|[a-f0-9]{64})$" => valid_git_commit(value),
        _ => false,
    }
}

fn schema_type_matches(kind: &str, value: &Value) -> bool {
    match kind {
        "object" => value.is_object(),
        "array" => value.is_array(),
        "string" => value.is_string(),
        "boolean" => value.is_boolean(),
        // Direct Value callers can assess only the represented value. Receipt
        // ingress must use numeric::receipt_value first for lexical exactness.
        "integer" => {
            value.as_i64().is_some()
                || value.as_u64().is_some()
                || value
                    .as_f64()
                    .is_some_and(|n| n.is_finite() && n.fract() == 0.0)
        }
        "number" => value.is_number(),
        _ => false,
    }
}

fn validate_schema_instance(
    schema: &Value,
    instance: &Value,
    root: &Value,
    where_: &str,
) -> Result<()> {
    if schema == &Value::Bool(true) {
        return Ok(());
    }
    if schema == &Value::Bool(false) {
        return Err(format!("{where_}: rejected by false schema"));
    }
    let object = schema
        .as_object()
        .ok_or("schema must be object or boolean")?;
    if let Some(reference) = object.get("$ref").and_then(Value::as_str) {
        let name = reference.trim_start_matches("#/$defs/");
        validate_schema_instance(&root["$defs"][name], instance, root, where_)?;
    }
    if let Some(kind) = object.get("type").and_then(Value::as_str) {
        ensure(
            schema_type_matches(kind, instance),
            &format!("{where_}: expected JSON type {kind}"),
        )?;
    }
    if let Some(expected) = object.get("const") {
        ensure(instance == expected, &format!("{where_}: const mismatch"))?;
    }
    if let Some(values) = object.get("enum").and_then(Value::as_array) {
        ensure(
            values.contains(instance),
            &format!("{where_}: enum mismatch"),
        )?;
    }
    if let Some(pattern) = object.get("pattern").and_then(Value::as_str) {
        let text = instance
            .as_str()
            .ok_or_else(|| format!("{where_}: pattern requires string"))?;
        ensure(
            schema_pattern_matches(pattern, text),
            &format!("{where_}: pattern mismatch"),
        )?;
    }
    if let Some(minimum) = object.get("minimum").and_then(Value::as_f64) {
        ensure(
            instance.as_f64().is_some_and(|value| value >= minimum),
            &format!("{where_}: minimum violated"),
        )?;
    }
    if let Some(min_length) = object.get("minLength").and_then(Value::as_u64) {
        ensure(
            instance
                .as_str()
                .is_some_and(|s| s.chars().count() as u64 >= min_length),
            &format!("{where_}: minLength violated"),
        )?;
    }
    if let Some(array) = instance.as_array() {
        if let Some(min_items) = object.get("minItems").and_then(Value::as_u64) {
            ensure(
                array.len() as u64 >= min_items,
                &format!("{where_}: minItems violated"),
            )?;
        }
        if object.get("uniqueItems") == Some(&Value::Bool(true)) {
            for index in 0..array.len() {
                ensure(
                    !array[index + 1..].contains(&array[index]),
                    &format!("{where_}: uniqueItems violated"),
                )?;
            }
        }
        if let Some(items) = object.get("items") {
            for (index, item) in array.iter().enumerate() {
                validate_schema_instance(items, item, root, &format!("{where_}/{index}"))?;
            }
        }
    }
    if let Some(instance_object) = instance.as_object() {
        if let Some(required) = object.get("required").and_then(Value::as_array) {
            for key in required.iter().filter_map(Value::as_str) {
                ensure(
                    instance_object.contains_key(key),
                    &format!("{where_}: required field {key} is absent"),
                )?;
            }
        }
        let properties = object.get("properties").and_then(Value::as_object);
        if object.get("additionalProperties") == Some(&Value::Bool(false)) {
            for key in instance_object.keys() {
                ensure(
                    properties.is_some_and(|properties| properties.contains_key(key)),
                    &format!("{where_}: unexpected property {key}"),
                )?;
            }
        }
        for (key, value) in instance_object {
            if let Some(property_schema) = properties.and_then(|properties| properties.get(key)) {
                validate_schema_instance(property_schema, value, root, &format!("{where_}/{key}"))?;
            } else if let Some(extra_schema) = object.get("additionalProperties")
                && extra_schema != &Value::Bool(false)
                && extra_schema != &Value::Bool(true)
            {
                validate_schema_instance(extra_schema, value, root, &format!("{where_}/{key}"))?;
            }
        }
    }
    if let Some(branches) = object.get("oneOf").and_then(Value::as_array) {
        let matching = branches
            .iter()
            .filter(|branch| validate_schema_instance(branch, instance, root, where_).is_ok())
            .count();
        ensure(
            matching == 1,
            &format!("{where_}: oneOf matched {matching} branches"),
        )?;
    }
    if let Some(branches) = object.get("allOf").and_then(Value::as_array) {
        for branch in branches {
            validate_schema_instance(branch, instance, root, where_)?;
        }
    }
    if let Some(condition) = object.get("if")
        && validate_schema_instance(condition, instance, root, where_).is_ok()
        && let Some(then_schema) = object.get("then")
    {
        validate_schema_instance(then_schema, instance, root, where_)?;
    }
    Ok(())
}

fn validate_schema(path: &str, value: &Value, required: &[&str]) -> Result<()> {
    ensure(
        value["$schema"]
            .as_str()
            .is_some_and(|s| s.contains("2020-12")),
        &format!("{path} must declare JSON Schema 2020-12"),
    )?;
    ensure(
        value["type"].as_str() == Some("object"),
        &format!("{path} root must be an object"),
    )?;
    ensure(
        value["additionalProperties"] == false,
        &format!("{path} must reject unknown fields"),
    )?;
    ensure(
        value["properties"]["schema_version"]["const"].as_str() == Some("1.0.0"),
        &format!("{path} has unsupported schema version"),
    )?;
    let keys: BTreeSet<_> = value["required"]
        .as_array()
        .ok_or("schema required fields missing")?
        .iter()
        .filter_map(Value::as_str)
        .collect();
    for field in required {
        ensure(
            keys.contains(field),
            &format!("{path} missing required field {field}"),
        )?;
    }
    validate_schema_tree(value, value, path)
}

pub fn check(root: &Path, records: &mut Vec<Value>) -> Result<()> {
    let inventory_path = root.join("docs/rust-rewrite/inventory/o03-active-tooling.json");
    let inventory: Value = serde_json::from_slice(
        &fs::read(&inventory_path).map_err(|e| format!("missing O03 inventory: {e}"))?,
    )
    .map_err(|e| e.to_string())?;
    let mut active_paths = BTreeSet::new();
    for entry in fs::read_dir(root.join("scripts")).map_err(|e| e.to_string())? {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path.extension().and_then(|x| x.to_str()) == Some("py") {
            active_paths.insert(format!(
                "scripts/{}",
                path.file_name()
                    .and_then(|x| x.to_str())
                    .ok_or("non-UTF8 script name")?
            ));
        }
    }
    validate_inventory(&inventory, &active_paths)?;
    check_supporting_paths(root, &inventory, records)?;
    let baseline = inventory["baseline_sha"]
        .as_str()
        .ok_or("O03 source baseline missing")?;
    ensure(
        baseline.len() == 40 && baseline.bytes().all(|b| b.is_ascii_hexdigit()),
        "O03 source baseline is not a Git commit SHA",
    )?;
    command(
        root,
        "git",
        &["merge-base", "--is-ancestor", baseline, "HEAD"],
        records,
    )?;
    check_source_hashes(root, &inventory, records)?;
    let disposition: Value = serde_json::from_slice(
        &fs::read(root.join("docs/rust-rewrite/inventory/python-disposition.json"))
            .map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let disposition_rows: std::collections::BTreeMap<_, _> = disposition["files"]
        .as_array()
        .ok_or("F01 files missing")?
        .iter()
        .filter_map(|row| Some((row["path"].as_str()?, row)))
        .collect();
    for row in inventory["scripts"].as_array().ok_or("scripts missing")? {
        let path = row["path"].as_str().ok_or("path missing")?;
        let owner = row["owner"].as_str().ok_or("owner missing")?;
        let f01 = disposition_rows
            .get(path)
            .ok_or("O03 script absent from F01 inventory")?;
        ensure(
            f01["classification"] == "backend-coupled tooling/test",
            &format!("O03 {path} is not classified as backend-coupled in F01"),
        )?;
        ensure(
            f01["migration_owners"]
                .as_array()
                .is_some_and(|owners| owners.iter().any(|v| v.as_str() == Some(owner))),
            &format!("O03 {path} owner differs from F01"),
        )?;
    }
    for (file, required) in [
        (
            "runtime-receipt.schema.json",
            [
                "schema_version",
                "receipt_kind",
                "binary",
                "source",
                "toolchain",
                "configuration",
                "authority",
                "clock",
                "reservation",
                "accounting",
                "outcome",
            ]
            .as_slice(),
        ),
        (
            "source-receipt.schema.json",
            [
                "schema_version",
                "receipt_kind",
                "git_commit",
                "dirty",
                "tree_sha256",
                "files",
                "binary",
                "toolchain",
                "configuration_sha256",
            ]
            .as_slice(),
        ),
        (
            "qualification-result.schema.json",
            [
                "schema_version",
                "runtime_receipt_sha256",
                "checks",
                "outcome",
                "source_sha256",
            ]
            .as_slice(),
        ),
    ] {
        let path = format!("contracts/o03/{file}");
        let schema: Value = serde_json::from_slice(
            &fs::read(root.join(&path)).map_err(|e| format!("missing {path}: {e}"))?,
        )
        .map_err(|e| format!("invalid {path}: {e}"))?;
        validate_schema(&path, &schema, required)?;
    }
    validate_contract_fixtures(root, records)?;
    records.push(serde_json::json!({"check":"o03_inventory_and_contracts","script_count":active_paths.len(),"owners":{"O07":inventory["scripts"].as_array().unwrap().iter().filter(|r|r["owner"]=="O07").count(),"O08":inventory["scripts"].as_array().unwrap().iter().filter(|r|r["owner"]=="O08").count()},"provider_calls":0,"status":"passed"}));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_hash_check_accepts_only_exact_or_authenticated_successors() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let current = "scripts/verify_docker_base_digest.py";
        let prior = "3f43f9bc1f90b4b2cd85381d2a54e54387cfab18";
        let exact = command(&root, "git", &["hash-object", "--", current], &mut vec![])
            .unwrap()
            .trim()
            .to_string();
        assert_ne!(exact, prior);
        assert!(source_hash_matches(&root, current, &exact, &exact).unwrap());
        assert!(source_hash_matches(&root, current, prior, &exact).unwrap());
        assert!(!source_hash_matches(&root, "scripts/other.py", prior, "different").unwrap());
        assert!(source_hash_matches(&root, current, "wrong", &exact).is_err());
    }
    fn valid() -> Value {
        serde_json::json!({"schema_version":"1.0.0","inventory_kind":"active_scripts_directory_exact_set","scripts":[{"path":"scripts/a.py","source_blob":"a".repeat(40),"current_entrypoint":"python scripts/a.py","entrypoint_kind":"cli","entrypoint_functions":["main"],"api_signatures":[{"name":"main","arguments":[],"async":false,"return_annotation":"int"}],"cli_options":["--fixture"],"source_summary":"runs a bounded acquisition","inputs":["fixture"],"outputs":["receipt"],"internal_imports":[],"direct_backend_imports":[],"consumed_by":["b"],"native_successor":"ssx experiment acquire --plan <id> --input <path> --output <path>","boundary":"native CLI; no Python backend imports","disposition":"backend_coupled","owner":"O07","stage":"acquisition","handoff_validation":"offline fixtures and bounds"},{"path":"scripts/b.py","source_blob":"b".repeat(40),"current_entrypoint":"programmatic API: scripts.b","entrypoint_kind":"programmatic_module","entrypoint_functions":["verify"],"api_signatures":[{"name":"verify","arguments":["path"],"async":false,"return_annotation":"bool"}],"cli_options":[],"source_summary":"verifies receipts","inputs":["receipt"],"outputs":["result"],"internal_imports":["a"],"direct_backend_imports":["slopsearx.service"],"consumed_by":[],"native_successor":"ssx evidence verify --input <path> --output <path>","boundary":"native CLI; no Python backend imports","disposition":"backend_coupled","owner":"O08","stage":"verification","handoff_validation":"offline fixtures and negative cases"}]})
    }
    #[test]
    fn exact_inventory_requires_complete_ownership_and_native_boundaries() {
        let paths = BTreeSet::from(["scripts/a.py".to_string(), "scripts/b.py".to_string()]);
        assert!(validate_inventory(&valid(), &paths).is_ok());
        assert!(
            validate_inventory(&valid(), &BTreeSet::from(["scripts/a.py".to_string()])).is_err()
        );
        let mut missing_owner = valid();
        missing_owner["scripts"][0]
            .as_object_mut()
            .unwrap()
            .remove("owner");
        assert!(validate_inventory(&missing_owner, &paths).is_err());
        let mut unsupported = valid();
        unsupported["schema_version"] = "2.0.0".into();
        assert!(validate_inventory(&unsupported, &paths).is_err());
        let mut misclassified = valid();
        misclassified["scripts"][0]["boundary"] = "Python imports backend".into();
        assert!(validate_inventory(&misclassified, &paths).is_err());
        let mut wrong_owner = valid();
        wrong_owner["scripts"][1]["owner"] = "O03".into();
        assert!(validate_inventory(&wrong_owner, &paths).is_err());
    }
    #[test]
    fn receipt_contract_rejects_missing_binding_fields_and_bad_versions() {
        let object =
            serde_json::json!({"type":"object","additionalProperties":false,"properties":{}});
        let base = serde_json::json!({"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","additionalProperties":false,"required":["schema_version","binary","source","toolchain","configuration","authority","clock","reservation","accounting","outcome"],"properties":{"schema_version":{"const":"1.0.0"},"binary":object,"source":object,"toolchain":object,"configuration":object,"authority":object,"clock":object,"reservation":object,"accounting":object,"outcome":{"type":"string"}}});
        assert!(
            validate_schema(
                "test",
                &base,
                &[
                    "binary",
                    "source",
                    "toolchain",
                    "configuration",
                    "authority",
                    "clock",
                    "reservation",
                    "accounting",
                    "outcome"
                ]
            )
            .is_ok()
        );
        let mut incomplete = base.clone();
        incomplete["required"] = serde_json::json!(["schema_version"]);
        assert!(validate_schema("test", &incomplete, &["binary"]).is_err());
        let mut bad_version = base;
        bad_version["properties"]["schema_version"]["const"] = "0.1".into();
        assert!(validate_schema("test", &bad_version, &["binary"]).is_err());
        let mut impossible = serde_json::json!({"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","additionalProperties":false,"required":["schema_version","binary"],"properties":{"schema_version":{"const":"1.0.0"}}});
        assert!(validate_schema("test", &impossible, &["schema_version"]).is_err());
        impossible["properties"]["binary"] = serde_json::json!({"type":"object"});
        assert!(validate_schema("test", &impossible, &["schema_version", "binary"]).is_err());
        impossible["properties"]["binary"] =
            serde_json::json!({"type":"object","additionalProperties":false,"properties":{}});
        assert!(validate_schema("test", &impossible, &["schema_version", "binary"]).is_ok());

        let mut unsupported_root = impossible.clone();
        unsupported_root["maxLength"] = 1.into();
        assert!(validate_schema("test", &unsupported_root, &["binary"]).is_err());
        let mut unsupported_property = impossible.clone();
        unsupported_property["properties"]["binary"]["properties"]["field"] =
            serde_json::json!({"maxLength":1});
        assert!(validate_schema("test", &unsupported_property, &["binary"]).is_err());
        let mut unsupported_combinator = impossible.clone();
        unsupported_combinator["allOf"] =
            serde_json::json!([{"properties":{"field":{"maxLength":1}}}]);
        assert!(validate_schema("test", &unsupported_combinator, &["binary"]).is_err());
        let mut unsupported_type_form = impossible;
        unsupported_type_form["properties"]["binary"]["type"] = serde_json::json!(["object"]);
        assert!(validate_schema("test", &unsupported_type_form, &["binary"]).is_err());
    }

    #[test]
    fn git_commit_ids_accept_only_sha1_or_sha256_lengths_in_schema_and_semantics() {
        let pattern = "^([a-f0-9]{40}|[a-f0-9]{64})$";
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "git_commit": {"type": "string", "pattern": pattern}
            }
        });
        for length in [40, 64] {
            let commit = "a".repeat(length);
            let instance = serde_json::json!({"git_commit": commit});
            assert!(valid_git_commit(&"a".repeat(length)));
            assert!(validate_schema_instance(&schema, &instance, &schema, "receipt").is_ok());
        }
        for length in [39, 41, 50, 63, 65] {
            let commit = "a".repeat(length);
            let instance = serde_json::json!({"git_commit": commit});
            assert!(!valid_git_commit(&"a".repeat(length)));
            assert!(validate_schema_instance(&schema, &instance, &schema, "receipt").is_err());
        }
    }

    #[test]
    fn expired_undispatched_denial_can_record_terminal_elapsed_time() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let bytes =
            fs::read(root.join("crates/xtask/fixtures/o03/runtime-receipt.accepted.json")).unwrap();
        let mut blocked = numeric::receipt_value(&bytes).unwrap();
        blocked["authority"]["status"] = serde_json::json!("denied");
        blocked["outcome"] = serde_json::json!("blocked");
        blocked["clock"]["started_unix_ms"] = serde_json::json!(1001);
        blocked["clock"]["finished_unix_ms"] = serde_json::json!(1011);
        blocked["clock"]["monotonic_elapsed_ms"] = serde_json::json!(10);
        blocked["accounting"]["requests_dispatched"] = serde_json::json!(0);
        blocked["accounting"]["requests_settled"] = serde_json::json!(0);
        blocked["accounting"]["requests_released"] = serde_json::json!(2);
        validate_runtime_receipt(blocked.clone()).unwrap();
        // The terminal-record exception cannot authorize any actual dispatch.
        let mut dispatched = blocked.clone();
        dispatched["authority"]["status"] = serde_json::json!("authorized");
        dispatched["accounting"]["requests_dispatched"] = serde_json::json!(1);
        dispatched["accounting"]["requests_settled"] = serde_json::json!(1);
        dispatched["accounting"]["requests_released"] = serde_json::json!(1);
        assert!(validate_runtime_receipt(dispatched).is_err());
        for (pointer, value) in [
            ("/clock/finished_unix_ms", serde_json::json!(1000)),
            ("/reservation/deadline_unix_ms", serde_json::json!(999)),
            ("/accounting/requests_released", serde_json::json!(1)),
            ("/accounting/requests_unresolved", serde_json::json!(1)),
            ("/accounting/cost_microusd_reserved", serde_json::json!(101)),
            ("/outcome", serde_json::json!("accepted")),
            ("/authority/status", serde_json::json!("authorized")),
        ] {
            let mut invalid = blocked.clone();
            *invalid.pointer_mut(pointer).unwrap() = value;
            assert!(validate_runtime_receipt(invalid).is_err(), "{pointer}");
        }
    }

    #[test]
    fn source_paths_reject_cross_platform_traversal_and_aliases() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let bytes =
            fs::read(root.join("crates/xtask/fixtures/o03/source-receipt.valid.json")).unwrap();
        let original = numeric::receipt_value(&bytes).unwrap();
        for path in [
            r"..\outside",
            r"src\..\outside",
            r"C:\outside",
            r"C:outside",
            r"\\server\share\file",
            "/outside",
            "src/../outside",
            "src/./file",
            "src//file",
            "src/",
            "src/.. /outside",
            "src/file.",
            "src/file ",
            "src/fi\0le",
            "\t\n",
            "   ",
        ] {
            let mut receipt = original.clone();
            receipt["files"][0]["path"] = Value::String(path.into());
            assert!(validate_source_receipt(receipt).is_err(), "{path:?}");
        }
        let inventory: Value = serde_json::from_slice(
            &fs::read(root.join("docs/rust-rewrite/inventory/o03-active-tooling.json")).unwrap(),
        )
        .unwrap();
        for row in inventory["scripts"].as_array().unwrap() {
            assert!(portable_source_path(row["path"].as_str().unwrap()));
        }
        for path in ["src/lib.rs", ".github/workflows/rust.yml", "雪/source.rs"] {
            let mut receipt = original.clone();
            receipt["files"][0]["path"] = Value::String(path.into());
            validate_source_receipt(receipt).unwrap();
        }
    }

    #[test]
    fn schema_integer_accepts_integral_decimal_representation() {
        let value: Value = serde_json::from_str("1.0").unwrap();
        assert!(schema_type_matches("integer", &value));
    }

    #[test]
    fn receipt_ingress_accepts_decimal_and_exponent_spellings_before_typed_decode() {
        fn spell(value: &Value, exponent: bool) -> String {
            match value {
                Value::Number(n) => format!("{n}{}", if exponent { "e0" } else { ".0" }),
                Value::Array(a) => format!(
                    "[{}]",
                    a.iter()
                        .map(|v| spell(v, exponent))
                        .collect::<Vec<_>>()
                        .join(",")
                ),
                Value::Object(o) => format!(
                    "{{{}}}",
                    o.iter()
                        .map(|(k, v)| format!(
                            "{}:{}",
                            serde_json::to_string(k).unwrap(),
                            spell(v, exponent)
                        ))
                        .collect::<Vec<_>>()
                        .join(",")
                ),
                _ => serde_json::to_string(value).unwrap(),
            }
        }
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        for (file, schema, validate) in [
            (
                "source-receipt.valid.json",
                "source-receipt.schema.json",
                validate_source_receipt as fn(Value) -> Result<()>,
            ),
            (
                "runtime-receipt.accepted.json",
                "runtime-receipt.schema.json",
                validate_runtime_receipt,
            ),
        ] {
            let bytes = fs::read(root.join("crates/xtask/fixtures/o03").join(file)).unwrap();
            let original: Value = serde_json::from_slice(&bytes).unwrap();
            let schema: Value =
                serde_json::from_slice(&fs::read(root.join("contracts/o03").join(schema)).unwrap())
                    .unwrap();
            for exponent in [false, true] {
                let spelled = spell(&original, exponent);
                let normalized = numeric::receipt_value(spelled.as_bytes()).unwrap();
                assert_eq!(normalized, original);
                validate_schema_instance(&schema, &normalized, &schema, "receipt").unwrap();
                validate(normalized).unwrap();
                // Normalization cannot substitute canonical bytes for exact receipt binding.
                assert_ne!(sha256_hex(spelled.as_bytes()), sha256_hex(&bytes));
            }
        }
    }

    #[test]
    fn prospective_receipt_fixtures_cover_success_unknown_and_invariant_failures() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut records = Vec::new();
        assert!(validate_contract_fixtures(&root, &mut records).is_ok());
        assert_eq!(records[0]["valid_examples"], 5);
        assert_eq!(records[0]["negative_examples"], 19);
    }
}
