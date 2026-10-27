//! F06 source-bound configuration and engine catalog gate.
use crate::{Result, command};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use ssx_registry::{AuthClass, EngineDescriptor, ImplementationStatus, Registry, RoutingRole};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Pins {
    schema_version: u32,
    plan_id: String,
    baseline: String,
    sources: Vec<Pin>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Pin {
    path: String,
    git_blob: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RoutingExpectations {
    schema_version: u32,
    plan_id: String,
    source: String,
    source_git_blob: String,
    engines: Vec<RoutingExpectation>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RoutingExpectation {
    name: String,
    role: RoutingRole,
    purpose: Option<String>,
    use_when: Option<String>,
}

fn read_json(path: &Path) -> Result<Value> {
    serde_json::from_slice(&fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?)
        .map_err(|error| format!("{}: {error}", path.display()))
}

// This documentation-only successor must reconstruct exactly from the frozen
// README; no provider/source expectations or historical pins are rewritten.
const NATIVE_README_SECTION: &str = "## Engines\n\n### Rust-only additions\n\n| Engine | Type | Auth | Categories |\n|---|---|---|---|\n| [DeepSeek Search](docs/DEEPSEEK.md) (opt-in, Rust only) | API | `ENGINE_DEEPSEEK_API_KEY` | general |\n\nDeepSeek extends the native registry during the Rust rewrite. It is disabled by\ndefault and does not change the 51-engine Python migration baseline below.\n\n### Existing engines (51)";

fn native_readme_successor(baseline: &str, current: &str) -> bool {
    baseline.matches("## Engines (51)").count() == 1
        && baseline.replacen("## Engines (51)", NATIVE_README_SECTION, 1) == current
}

fn source_pins(root: &Path, records: &mut Vec<Value>) -> Result<()> {
    let path = root.join("docs/rust-rewrite/f06/source-pins.json");
    let pins: Pins = serde_json::from_slice(
        &fs::read(&path).map_err(|error| format!("F06 source pins missing: {error}"))?,
    )
    .map_err(|_| "F06 source pins malformed".to_string())?;
    if pins.schema_version != 1
        || pins.plan_id != "F06"
        || pins.baseline.len() != 40
        || pins.sources.is_empty()
    {
        return Err("invalid F06 source pin header".into());
    }
    let mut seen = BTreeSet::new();
    for pin in &pins.sources {
        if pin.path.is_empty()
            || Path::new(&pin.path).is_absolute()
            || pin.path.split('/').any(|part| part == "..")
            || pin.git_blob.len() != 40
            || !pin.git_blob.bytes().all(|byte| byte.is_ascii_hexdigit())
            || !seen.insert(pin.path.as_str())
        {
            return Err("invalid or duplicate F06 source pin".into());
        }
        let actual = command(root, "git", &["hash-object", "--", &pin.path], records)?;
        if actual.trim() != pin.git_blob {
            let reviewed_readme = if pin.path == "README.md"
                && pin.git_blob == "3b50b38382be7e918a3d0bf4edf068a674b699b5"
            {
                let baseline = command(root, "git", &["cat-file", "blob", &pin.git_blob], records)?;
                let current =
                    fs::read_to_string(root.join(&pin.path)).map_err(|error| error.to_string())?;
                native_readme_successor(&baseline, &current)
            } else {
                false
            };
            if !reviewed_readme {
                return Err(format!("stale F06 source evidence: {}", pin.path));
            }
            records.push(json!({"kind":"native_readme_successor","baseline_git_blob":pin.git_blob,"path":pin.path,"exact_reconstruction":true}));
        }
    }
    let tracked = command(root, "git", &["ls-files", "-z"], records)?;
    let expected: BTreeSet<String> = tracked
        .split('\0')
        .filter(|path| {
            path.starts_with("engines/") && path.ends_with(".py")
                || matches!(
                    *path,
                    "slopsearx/config.py"
                        | "slopsearx/adapter.py"
                        | "slopsearx/capabilities.py"
                        | "slopsearx/jev.py"
                        | "engines/__init__.py"
                        | "README.md"
                        | "docs/ENGINE_ADAPTERS.md"
                )
        })
        .map(str::to_owned)
        .collect();
    let provided: BTreeSet<String> = pins.sources.iter().map(|pin| pin.path.clone()).collect();
    if expected != provided {
        return Err(
            "F06 source pin coverage differs from all registered source/config/docs files".into(),
        );
    }
    Ok(())
}

fn catalog(root: &Path) -> Result<Vec<EngineDescriptor>> {
    let fragment_dir = root.join("docs/rust-rewrite/f06/fragments");
    let mut fragments = Vec::new();
    let mut paths = BTreeSet::new();
    for entry in
        fs::read_dir(&fragment_dir).map_err(|error| format!("F06 fragments missing: {error}"))?
    {
        let path = entry.map_err(|error| error.to_string())?.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let descriptor: EngineDescriptor =
            serde_json::from_slice(&fs::read(&path).map_err(|error| error.to_string())?)
                .map_err(|_| format!("invalid F06 fragment: {}", path.display()))?;
        if path.file_stem().and_then(|stem| stem.to_str()) != Some(descriptor.name.as_str())
            || !paths.insert(descriptor.name.clone())
        {
            return Err(format!(
                "duplicate or misnamed F06 fragment: {}",
                path.display()
            ));
        }
        fragments.push(descriptor);
    }
    fragments.sort_by(|left, right| left.name.cmp(&right.name));
    let manifest: Vec<EngineDescriptor> = serde_json::from_slice(
        &fs::read(root.join("docs/rust-rewrite/f06/catalog.json"))
            .map_err(|error| error.to_string())?,
    )
    .map_err(|_| "F06 generated catalog malformed".to_string())?;
    let mut sorted_manifest = manifest.clone();
    sorted_manifest.sort_by(|left, right| left.name.cmp(&right.name));
    if fragments != sorted_manifest {
        return Err("generated F06 catalog differs from engine-local fragments".into());
    }
    if fragments.len() != 51 {
        return Err(format!(
            "expected 51 engine descriptors, found {}",
            fragments.len()
        ));
    }
    let names: BTreeSet<_> = fragments.iter().map(|item| item.name.as_str()).collect();
    if names.len() != 51 {
        return Err("engine descriptor names are not unique".into());
    }
    for descriptor in &fragments {
        ssx_registry::validate_descriptor(descriptor).map_err(|error| error.to_string())?;
        if descriptor.implementation != ImplementationStatus::Unavailable {
            return Err(format!(
                "unported engine status must be explicit: {}",
                descriptor.name
            ));
        }
        if descriptor.auth_class == AuthClass::Required && descriptor.api_key_env.is_empty() {
            return Err(format!(
                "required auth variable missing: {}",
                descriptor.name
            ));
        }
        if descriptor.routing.role == RoutingRole::Specialist
            && descriptor
                .routing
                .purpose
                .as_deref()
                .is_none_or(str::is_empty)
        {
            return Err(format!("specialist purpose missing: {}", descriptor.name));
        }
    }
    let counts = fragments.iter().fold(BTreeMap::new(), |mut counts, item| {
        *counts.entry(item.routing.role).or_insert(0usize) += 1;
        counts
    });
    if counts.get(&RoutingRole::General) != Some(&3)
        || counts.get(&RoutingRole::BroadSupport) != Some(&4)
        || counts.get(&RoutingRole::Specialist) != Some(&44)
    {
        return Err(format!(
            "routing role counts differ from frozen source: {counts:?}"
        ));
    }
    if fragments
        .iter()
        .filter(|item| item.defaults.enabled)
        .count()
        != 49
    {
        return Err("default enabled/opt-in engine set changed".into());
    }
    Ok(manifest)
}

fn validate_routing_expectations(root: &Path, descriptors: &[EngineDescriptor]) -> Result<()> {
    let path = root.join("docs/rust-rewrite/f06/routing-expectations.json");
    let expectations: RoutingExpectations = serde_json::from_slice(
        &fs::read(&path).map_err(|error| format!("F06 routing expectations missing: {error}"))?,
    )
    .map_err(|_| "F06 routing expectations malformed".to_string())?;
    if expectations.schema_version != 1
        || expectations.plan_id != "F06"
        || expectations.source
            != "slopsearx/jev.py::ROUTING_CARDS/GENERAL_ENGINES/BROAD_SUPPORT_ENGINES"
    {
        return Err("invalid F06 routing expectation provenance".into());
    }
    let pins: Pins = serde_json::from_slice(
        &fs::read(root.join("docs/rust-rewrite/f06/source-pins.json"))
            .map_err(|error| format!("F06 source pins missing: {error}"))?,
    )
    .map_err(|_| "F06 source pins malformed".to_string())?;
    let jev_pin = pins
        .sources
        .iter()
        .find(|pin| pin.path == "slopsearx/jev.py")
        .ok_or("F06 source pins omit slopsearx/jev.py")?;
    if expectations.source_git_blob != jev_pin.git_blob {
        return Err("F06 routing fixture is not pinned to the source Jev catalog".into());
    }

    let expected: BTreeMap<_, _> = expectations
        .engines
        .iter()
        .map(|item| (item.name.as_str(), item))
        .collect();
    if expected.len() != expectations.engines.len() || expected.len() != descriptors.len() {
        return Err("F06 per-engine routing expectation set is incomplete or duplicated".into());
    }
    for descriptor in descriptors {
        let item = expected
            .get(descriptor.name.as_str())
            .ok_or_else(|| format!("missing frozen routing card: {}", descriptor.name))?;
        if descriptor.routing.role != item.role
            || descriptor.routing.purpose != item.purpose
            || descriptor.routing.use_when != item.use_when
        {
            return Err(format!(
                "F06 routing metadata differs from frozen source: {}",
                descriptor.name
            ));
        }
    }
    Ok(())
}

fn generated_markdown(descriptors: &[EngineDescriptor]) -> String {
    let mut output = String::from(
        "# Native engine catalog (F06)\n\nGenerated from `catalog.json`, checked against 51 engine-local fragments.\n\n| Name | Type | Categories | Auth | Cost | Jev role | Implementation |\n| --- | --- | --- | --- | --- | --- | --- |\n",
    );
    for engine in descriptors {
        let auth = match engine.auth_class {
            AuthClass::None => "none",
            AuthClass::Optional => "optional",
            AuthClass::Required => "required",
            AuthClass::Unknown => "unknown",
        };
        let cost = engine.cost_class.as_deref().unwrap_or("unknown");
        let role = match engine.routing.role {
            RoutingRole::General => "general",
            RoutingRole::BroadSupport => "broad_support",
            RoutingRole::Specialist => "specialist",
        };
        let categories = engine.categories.join(", ");
        let implementation = match engine.implementation {
            ImplementationStatus::Unavailable => "unavailable",
            ImplementationStatus::Available => "available",
        };
        output.push_str(&format!(
            "| `{}` | {} | {} | {} | {} | {} | {} |\n",
            engine.name, engine.engine_type, categories, auth, cost, role, implementation
        ));
    }
    output.push_str("\n`unavailable` means the descriptor is registered while the native provider port is not yet implemented. Secret values are never part of this catalog.\n");
    output
}

fn coverage_fixture(root: &Path) -> Result<()> {
    let value = read_json(&root.join("crates/xtask/fixtures/f06/cases.json"))?;
    if value["schema_version"] != 1 || value["plan_id"] != "F06" {
        return Err("invalid F06 behavior fixture header".into());
    }
    let cases = value["cases"]
        .as_array()
        .ok_or("F06 behavior fixture has no cases")?;
    let actual: BTreeSet<_> = cases
        .iter()
        .map(|case| {
            case["id"]
                .as_str()
                .ok_or_else(|| "F06 case has no id".to_string())
        })
        .collect::<Result<_>>()?;
    let expected: BTreeSet<_> = [
        "defaults",
        "empty_yaml",
        "partial_overlay",
        "custom_engine",
        "env_precedence",
        "empty_categories",
        "formats_and_globals",
        "hf_alias",
        "dual_credentials",
        "file_credentials",
        "bad_yaml_redaction",
        "invalid_numeric",
        "root_scalar",
        "registry_completeness",
        "routing_card_mapping",
        "routing_same_count_swap",
        "duplicate_registration",
        "fixture_adapter",
        "filter_layers",
        "auth_redaction",
    ]
    .into_iter()
    .collect();
    if actual != expected || cases.len() != expected.len() {
        return Err("F06 source behavior case coverage is incomplete or duplicated".into());
    }
    for case in cases {
        if case["source"].as_str().is_none_or(str::is_empty)
            || case["expected"].as_str().is_none_or(str::is_empty)
        {
            return Err("F06 behavior case lacks source observation or expected result".into());
        }
    }
    Ok(())
}

const SCALAR_CORPUS_SHA256: &str =
    "d248b7961f83a24e951af6db93e9f0f02526df386f608587904109def6c9eeab";
fn validate_scalar_corpus(raw: &[u8]) -> Result<Value> {
    let hash: String = Sha256::digest(raw)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    if hash != SCALAR_CORPUS_SHA256 {
        return Err("F06 reviewed scalar capture mismatch".into());
    }
    let data: Value = serde_json::from_slice(raw).map_err(|error| error.to_string())?;
    if data["schema_version"] != 1
        || data["python"] != "3.14.7"
        || data["baseline"] != "7cc420f7ffb61127750e77bf6130e225925f9186"
        || data["cases"].as_array().map(Vec::len) != Some(21)
    {
        return Err("F06 scalar capture provenance mismatch".into());
    }
    Ok(data)
}
const CREDENTIAL_CORPUS_SHA256: &str =
    "8252ac8e98611e12713c0e68fb0a74a66d3453552e92b2482b0acbc204889bf7";
fn validate_credential_corpus(raw: &[u8]) -> Result<Value> {
    let hash: String = Sha256::digest(raw)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    if hash != CREDENTIAL_CORPUS_SHA256 {
        return Err("F06 reviewed credential capture mismatch".into());
    }
    let data: Value = serde_json::from_slice(raw).map_err(|error| error.to_string())?;
    if data["schema_version"] != 1
        || data["python"] != "3.14.7"
        || data["baseline"] != "7cc420f7ffb61127750e77bf6130e225925f9186"
        || data["cases"].as_array().map(Vec::len) != Some(10)
    {
        return Err("F06 credential capture provenance mismatch".into());
    }
    Ok(data)
}
const YAML_CORPUS_SHA256: &str = "28b483215affabba971a6c9f0aa8e31f6dbb86589beee52f91ef5bbac4c48451";
fn validate_yaml_corpus(raw: &[u8]) -> Result<Value> {
    let hash: String = Sha256::digest(raw)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    if hash != YAML_CORPUS_SHA256 {
        return Err("F06 reviewed YAML capture mismatch".into());
    }
    let data: Value = serde_json::from_slice(raw).map_err(|error| error.to_string())?;
    if data["schema_version"] != 1
        || data["python"] != "3.14.7"
        || data["yaml_version"] != "6.0.3"
        || data["baseline"] != "7cc420f7ffb61127750e77bf6130e225925f9186"
        || data["cases"].as_array().map(Vec::len) != Some(79)
        || data["config_cases"].as_array().map(Vec::len) != Some(7)
    {
        return Err("F06 YAML capture provenance mismatch".into());
    }
    Ok(data)
}
pub fn check(root: &Path, records: &mut Vec<Value>) -> Result<()> {
    let scalar = validate_scalar_corpus(
        &fs::read(root.join("crates/xtask/fixtures/f06/scalars.json"))
            .map_err(|error| error.to_string())?,
    )?;
    let actual = command(
        root,
        "git",
        &["hash-object", "--", "slopsearx/config.py"],
        records,
    )?;
    if scalar["source_blob"].as_str() != Some(actual.trim()) {
        return Err("F06 scalar capture source drift".into());
    }

    let credentials = validate_credential_corpus(
        &fs::read(root.join("crates/xtask/fixtures/f06/credentials.json"))
            .map_err(|error| error.to_string())?,
    )?;
    if credentials["source_blob"].as_str() != Some(actual.trim()) {
        return Err("F06 credential capture source drift".into());
    }

    let yaml = validate_yaml_corpus(
        &fs::read(root.join("crates/xtask/fixtures/f06/yaml.json"))
            .map_err(|error| error.to_string())?,
    )?;
    if yaml["source_blob"].as_str() != Some(actual.trim()) {
        return Err("F06 YAML capture source drift".into());
    }

    source_pins(root, records)?;
    coverage_fixture(root)?;
    let descriptors = catalog(root)?;
    validate_routing_expectations(root, &descriptors)?;
    let mut swapped = descriptors.clone();
    let left = swapped
        .iter()
        .position(|item| item.name == "brave")
        .unwrap();
    let right = swapped
        .iter()
        .position(|item| item.name == "abuseipdb")
        .unwrap();
    let (left_card, right_card) = if left < right {
        let (before, after) = swapped.split_at_mut(right);
        (&mut before[left].routing, &mut after[0].routing)
    } else {
        let (before, after) = swapped.split_at_mut(left);
        (&mut after[0].routing, &mut before[right].routing)
    };
    std::mem::swap(left_card, right_card);
    let role_counts = |items: &[EngineDescriptor]| {
        items.iter().fold(BTreeMap::new(), |mut counts, item| {
            *counts.entry(item.routing.role).or_insert(0usize) += 1;
            counts
        })
    };
    if role_counts(&descriptors) != role_counts(&swapped)
        || validate_routing_expectations(root, &swapped).is_ok()
    {
        return Err("F06 routing gate does not reject a same-count per-engine card swap".into());
    }
    let registry = Registry::migration_baseline().map_err(|error| error.to_string())?;
    if registry.len() != descriptors.len()
        || registry
            .descriptors()
            .map(|item| &item.name)
            .collect::<BTreeSet<_>>()
            != descriptors
                .iter()
                .map(|item| &item.name)
                .collect::<BTreeSet<_>>()
    {
        return Err("compiled F06 registry and source manifest diverge".into());
    }
    // Native additions are not rewritten into the frozen Python parity inventory.
    let runtime = Registry::builtins().map_err(|error| error.to_string())?;
    let native: Vec<EngineDescriptor> = serde_json::from_slice(
        &fs::read(root.join("docs/rust-rewrite/native-engines/catalog.json"))
            .map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    if native.len() != 1
        || native[0].name != "deepseek"
        || native[0].enabled_by_default
        || native[0].defaults.enabled
        || native[0].routing.role != RoutingRole::General
        || native[0].defaults.max_results != 10
        || !native[0].enforced_filters.is_empty()
        || native[0].supported_filters.values().any(|value| *value)
        || runtime.len() != descriptors.len() + native.len()
        || descriptors
            .iter()
            .chain(native.iter())
            .any(|item| runtime.descriptor(&item.name) != Some(item))
    {
        return Err("native extension catalog is incomplete or changes migration defaults".into());
    }
    let expected_markdown = generated_markdown(&descriptors);
    let actual_markdown = fs::read_to_string(root.join("docs/rust-rewrite/f06/engine-catalog.md"))
        .map_err(|error| format!("F06 generated documentation missing: {error}"))?;
    if actual_markdown != expected_markdown {
        return Err("F06 engine catalog documentation is stale".into());
    }
    command(
        root,
        "cargo",
        &["test", "--locked", "-p", "ssx-config", "-p", "ssx-registry"],
        records,
    )?;
    records.push(json!({"kind":"f06_catalog","engines":descriptors.len(),"specialists":44,"generated_docs":"docs/rust-rewrite/f06/engine-catalog.md","source_pins":58}));
    Ok(())
}

#[cfg(test)]
mod scalar_capture_tests {
    use super::*;
    #[test]
    fn native_readme_amendment_allows_only_exact_reviewed_insertion() {
        let before = "before\n## Engines (51)\nafter\n";
        let current = before.replace("## Engines (51)", NATIVE_README_SECTION);
        assert!(native_readme_successor(before, &current));
        assert!(!native_readme_successor(before, before));
        assert!(!native_readme_successor(
            before,
            &(current.clone() + "unrelated")
        ));
        assert!(!native_readme_successor(
            before,
            &current.replace("ENGINE_DEEPSEEK_API_KEY", "SECRET")
        ));
        assert!(!native_readme_successor(
            before,
            &current.replace(
                "Rust-only additions",
                "Rust-only additions\nRust-only additions"
            )
        ));
        assert!(!native_readme_successor(
            "## Engines (51)\n## Engines (51)",
            &current
        ));
    }

    #[test]
    fn yaml_answers_cannot_redeclare_their_own_identity() {
        let raw = include_bytes!("../fixtures/f06/yaml.json");
        assert!(validate_yaml_corpus(raw).is_ok());
        let mut data: Value = serde_json::from_slice(raw).unwrap();
        data["cases"][0]["expected"] = json!({"value":12});
        assert!(validate_yaml_corpus(&serde_json::to_vec(&data).unwrap()).is_err());
        assert!(validate_yaml_corpus(&[]).is_err());
    }
    #[test]
    fn credential_answers_cannot_redeclare_their_own_identity() {
        let raw = include_bytes!("../fixtures/f06/credentials.json");
        assert!(validate_credential_corpus(raw).is_ok());
        let mut data: Value = serde_json::from_slice(raw).unwrap();
        data["cases"][0]["expected"] = json!({"api_key": null, "api_secret": null});
        assert!(validate_credential_corpus(&serde_json::to_vec(&data).unwrap()).is_err());
        assert!(validate_credential_corpus(&[]).is_err());
    }
    #[test]
    fn scalar_answers_cannot_redeclare_their_own_identity() {
        let raw = include_bytes!("../fixtures/f06/scalars.json");
        assert!(validate_scalar_corpus(raw).is_ok());
        let mut data: Value = serde_json::from_slice(raw).unwrap();
        data["cases"][0]["expected"] = json!({"error":"invalid"});
        assert!(validate_scalar_corpus(&serde_json::to_vec(&data).unwrap()).is_err());
        assert!(validate_scalar_corpus(&[]).is_err());
    }
}
