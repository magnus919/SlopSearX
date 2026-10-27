//! Source-reported identities, grouped before paging. No search or retrieval I/O.
use super::artifact;
use sha2::{Digest, Sha256};
use ssx_domain::{SearchResult, SearchSnapshot};
use ssx_json::{JsonProfile, Value, canonical_json, json};
use std::collections::{BTreeMap, BTreeSet};
use unicode_normalization::UnicodeNormalization;
type Identity = (String, Value);
fn text(v: &Value) -> Option<&str> {
    let s = v.as_str()?;
    (!s.is_empty()
        && s.chars().count() <= 512
        && !s
            .chars()
            .any(|c| ssx_json::python_whitespace(c) || (c as u32) < 32 || c == '\u{7f}'))
    .then_some(s)
}
fn context(r: &SearchResult) -> Result<(Value, BTreeSet<String>, String, Value), &'static str> {
    let p = ssx_domain::storage::encode_result(r)
        .ok()
        .and_then(|v| v.get("payload").cloned())
        .filter(|v| v["schema_version"].as_u64() == Some(1))
        .ok_or("missing_or_unsupported_payload")?;
    let data = p["data"].clone();
    let provenance = &p["provenance"];
    if !data.is_object() || !provenance.is_object() {
        return Err("invalid_payload");
    }
    let source = provenance["engine"]
        .as_str()
        .filter(|s| {
            if r.engines.is_empty() {
                *s == r.engine
            } else {
                r.engines.contains(*s)
            }
        })
        .ok_or("conflicting_provenance")?;
    let reported = provenance["adapter_fields"]
        .as_array()
        .ok_or("invalid_provenance")?;
    let inferred = provenance["inferred_fields"]
        .as_array()
        .ok_or("invalid_provenance")?;
    let fields = reported
        .iter()
        .filter(|f| !inferred.contains(f))
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect();
    Ok((data, fields, source.to_owned(), p))
}
fn ascii_name(s: &str) -> bool {
    s.bytes()
        .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
}
fn cve(s: &str) -> Option<String> {
    let s = s.to_uppercase();
    let mut a = s.split('-');
    let prefix = a.next()?;
    let year = a.next()?;
    let number = a.next()?;
    (prefix == "CVE"
        && year.len() == 4
        && year.bytes().all(|b| b.is_ascii_digit())
        && number.len() >= 4
        && number.bytes().all(|b| b.is_ascii_digit())
        && a.next().is_none())
    .then_some(s)
}
fn v1(r: &SearchResult) -> (Vec<Identity>, String, Value) {
    let (data, explicit, source, p) = match context(r) {
        Ok(v) => v,
        Err(reason) => return (vec![], reason.into(), json!({})),
    };
    if p["domain"] == "security" && p["type"] == "vulnerability" {
        let id = text(&data["cve_id"])
            .and_then(cve)
            .filter(|_| explicit.contains("cve_id"));
        return if let Some(id) = id {
            (
                vec![("cve".into(), json!({"cve_id":id}))],
                "explicit_adapter_identifier".into(),
                data,
            )
        } else {
            (vec![], "invalid_or_unreported_identifier".into(), json!({}))
        };
    }
    if p["domain"] == "packages" && p["type"] == "package" {
        if !["npm", "pypi"].contains(&source.as_str()) {
            return (vec![], "unknown_ecosystem".into(), json!({}));
        }
        if data
            .get("ecosystem")
            .is_some_and(|v| v.as_str() != Some(&source))
        {
            return (vec![], "conflicting_ecosystem".into(), json!({}));
        }
        let Some(name) = text(&data["name"]).filter(|_| explicit.contains("name")) else {
            return (vec![], "invalid_or_unreported_identifier".into(), json!({}));
        };
        if data.get("version").is_some()
            && (!explicit.contains("version") || text(&data["version"]).is_none())
        {
            return (vec![], "invalid_or_unreported_version".into(), json!({}));
        }
        let good = if source == "npm" {
            let name = if let Some(scoped) = name.strip_prefix('@') {
                let Some((scope, name)) = scoped.split_once('/') else {
                    return (vec![], "invalid_or_unreported_identifier".into(), json!({}));
                };
                if scope.is_empty() || !ascii_name(scope) {
                    return (vec![], "invalid_or_unreported_identifier".into(), json!({}));
                }
                name
            } else {
                name
            };
            !name.is_empty() && name.as_bytes()[0].is_ascii_alphanumeric() && ascii_name(name)
        } else {
            ascii_name(name)
                && name.as_bytes()[0].is_ascii_alphanumeric()
                && name.as_bytes()[name.len() - 1].is_ascii_alphanumeric()
        };
        if !good {
            return (vec![], "invalid_or_unreported_identifier".into(), json!({}));
        }
        let normalized = if source == "pypi" {
            let mut out = String::new();
            let mut separator = false;
            for ch in name.chars() {
                if matches!(ch, '-' | '_' | '.') {
                    if !separator {
                        out.push('-');
                    }
                    separator = true;
                } else {
                    out.push(ch.to_ascii_lowercase());
                    separator = false;
                }
            }
            out
        } else {
            name.into()
        };
        return (
            vec![(source, json!({"name":normalized,"version":data["version"]}))],
            "explicit_adapter_identifier".into(),
            data,
        );
    }
    (vec![], "unsupported_domain".into(), json!({}))
}
fn number(v: &Value, prefix: &str) -> Option<String> {
    let raw = text(v)?;
    let raw = if !prefix.is_empty() && raw.to_uppercase().starts_with(prefix) {
        &raw[prefix.len()..]
    } else {
        raw
    };
    (!raw.is_empty()
        && raw.len() <= 9
        && raw.bytes().all(|b| b.is_ascii_digit())
        && raw.bytes().any(|b| b != b'0'))
    .then(|| format!("{prefix}{raw}"))
}
fn strict_url_identifier<'a>(raw: &'a str, hosts: &[&str]) -> Option<&'a str> {
    if raw.contains("://") {
        let (scheme, rest) = raw.split_once("://")?;
        if !scheme.eq_ignore_ascii_case("https") {
            return None;
        }
        let (host, path) = rest.split_once('/').unwrap_or((rest, ""));
        if !hosts.iter().any(|h| host.eq_ignore_ascii_case(h)) || path.contains(['?', '#']) {
            return None;
        }
        Some(path.trim_matches('/'))
    } else if raw.starts_with("//") || raw.contains(':') {
        None
    } else {
        Some(raw)
    }
}
fn repository(v: &Value) -> Option<String> {
    let raw = strict_url_identifier(text(v)?, &["github.com", "www.github.com"])?;
    let raw = raw.strip_suffix(".git").unwrap_or(raw);
    let (owner, repo) = raw.split_once('/')?;
    if owner.is_empty()
        || owner.len() > 39
        || !owner.as_bytes()[0].is_ascii_alphanumeric()
        || !owner.as_bytes()[owner.len() - 1].is_ascii_alphanumeric()
        || !owner
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        || repo.is_empty()
        || repo.len() > 100
        || !ascii_name(repo)
    {
        return None;
    }
    Some(format!(
        "{}/{}",
        owner.to_ascii_lowercase(),
        repo.to_ascii_lowercase()
    ))
}
fn openalex(v: &Value) -> Option<String> {
    let raw = strict_url_identifier(text(v)?, &["openalex.org"])?.to_uppercase();
    let digits = raw.strip_prefix('W')?;
    (digits.len() >= 4
        && digits.len() <= 15
        && digits.as_bytes()[0] != b'0'
        && digits.bytes().all(|b| b.is_ascii_digit()))
    .then_some(raw)
}
fn unquote(raw: &str) -> String {
    let mut bytes = Vec::new();
    let mut i = 0;
    while i < raw.len() {
        let b = raw.as_bytes()[i];
        if b == b'%'
            && i + 2 < raw.len()
            && let Some(v) = raw.as_bytes()[i + 1]
                .is_ascii_hexdigit()
                .then_some(raw.as_bytes()[i + 1])
                .and_then(|a| {
                    raw.as_bytes()[i + 2].is_ascii_hexdigit().then(|| {
                        ((a as char).to_digit(16).expect("hex") * 16
                            + (raw.as_bytes()[i + 2] as char).to_digit(16).expect("hex"))
                            as u8
                    })
                })
        {
            bytes.push(v);
            i += 3;
            continue;
        }
        bytes.push(b);
        i += 1;
    }
    String::from_utf8_lossy(&bytes).into_owned()
}
fn doi(v: &Value) -> Option<String> {
    let raw = text(v)?;
    let folded = ssx_json::casefold(raw);
    let mut decoded = None;
    for p in ["https://doi.org/", "http://doi.org/", "doi:"] {
        if folded.starts_with(p) {
            decoded = Some(unquote(&raw[p.len()..]));
            break;
        }
    }
    let raw = decoded.as_deref().unwrap_or(raw);
    let normalized = ssx_json::casefold(&raw.nfc().collect::<String>());
    let (prefix, suffix) = normalized.split_once('/')?;
    let digits = prefix.strip_prefix("10.")?;
    if !(4..=9).contains(&digits.len())
        || !digits.bytes().all(|b| b.is_ascii_digit())
        || suffix.is_empty()
        || normalized.chars().count() > 255
        || normalized
            .chars()
            .any(|c| ssx_json::python_whitespace(c) || (c as u32) < 32 || c == '\u{7f}')
    {
        return None;
    }
    Some(normalized)
}
fn advisory(v: &Value) -> Option<Identity> {
    let raw = text(v)?.to_uppercase();
    let p: Vec<_> = raw.split('-').collect();
    if p.len() == 4
        && p[0] == "GHSA"
        && p[1..]
            .iter()
            .all(|s| s.len() == 4 && s.bytes().all(|b| b"23456789CFGHJMPQRVWX".contains(&b)))
    {
        return Some(("ghsa".into(), json!({"advisory_id":raw})));
    }
    if p.len() == 3
        && ["PYSEC", "RUSTSEC", "GO"].contains(&p[0])
        && p[1].len() == 4
        && (1..=8).contains(&p[2].len())
        && p[1..].iter().all(|s| s.bytes().all(|b| b.is_ascii_digit()))
    {
        return Some(("osv_advisory".into(), json!({"advisory_id":raw})));
    }
    None
}
fn v2(r: &SearchResult) -> (Vec<Identity>, String, Value) {
    let Ok((data, fields, source, p)) = context(r) else {
        return (vec![], "missing_or_unsupported_payload".into(), json!({}));
    };
    let (mut identities, reason, _) = v1(r);
    let mut add =
        |field: &str, namespace: &str, canonical: &str, normalize: fn(&Value) -> Option<String>| {
            if fields.contains(field)
                && let Some(v) = normalize(&data[field])
            {
                let mut id = json!({});
                id[canonical] = json!(v);
                identities.push((namespace.into(), id));
            }
        };
    if p["domain"] == "science" && p["type"] == "publication" {
        add("doi", "doi", "doi", doi);
        if source == "pubmed" && data.get("pmid").is_none() {
            add("publication_id", "pmid", "pmid", |v| number(v, ""));
        }
        add("pmid", "pmid", "pmid", |v| number(v, ""));
        add("pmcid", "pmcid", "pmcid", |v| number(v, "PMC"));
        add("openalex_id", "openalex", "openalex_id", openalex);
    }
    if (p["domain"] == "code" && p["type"] == "repository")
        || (p["domain"] == "packages" && p["type"] == "package")
    {
        if p["domain"] == "packages" {
            add(
                "repository_url",
                "github_repository",
                "repository",
                repository,
            );
            add("repository", "github_repository", "repository", repository);
        } else {
            add("repository", "github_repository", "repository", repository);
            add(
                "repository_url",
                "github_repository",
                "repository",
                repository,
            );
        }
    }
    if p["domain"] == "security"
        && ["vulnerability", "advisory"]
            .iter()
            .any(|t| p["type"].as_str() == Some(t))
    {
        for field in ["ghsa_id", "advisory_id"] {
            if fields.contains(field)
                && let Some(id) = advisory(&data[field])
            {
                identities.push(id);
            }
        }
    }
    let mut seen = BTreeSet::new();
    identities.retain(|v| {
        seen.insert(canonical_json(&json!(v), JsonProfile::AsciiCompact).expect("identity"))
    });
    let mut namespaces: BTreeMap<String, BTreeSet<Vec<u8>>> = BTreeMap::new();
    for (ns, id) in &identities {
        namespaces
            .entry(ns.clone())
            .or_default()
            .insert(canonical_json(id, JsonProfile::AsciiCompact).expect("identity"));
    }
    let conflicts: BTreeSet<_> = namespaces
        .into_iter()
        .filter(|(_, v)| v.len() > 1)
        .map(|(k, _)| k)
        .collect();
    identities.retain(|(ns, _)| !conflicts.contains(ns));
    let reason = if !identities.is_empty() {
        "explicit_adapter_identifier".into()
    } else if !conflicts.is_empty() {
        "conflicting_source_identifiers".into()
    } else if [
        "doi",
        "pmid",
        "pmcid",
        "openalex_id",
        "repository",
        "repository_url",
        "ghsa_id",
        "advisory_id",
    ]
    .iter()
    .any(|k| data.get(k).is_some())
    {
        "invalid_inferred_or_conflicting_identifier".into()
    } else if reason == "explicit_adapter_identifier" {
        "unsupported_identifier".into()
    } else {
        reason
    };
    (identities, reason, data)
}
type EntityEdges = BTreeMap<(String, String, String), Value>;
pub(crate) struct Collector {
    cursor: String,
    version: u8,
    groups: Vec<Value>,
    index: BTreeMap<String, usize>,
    facts: BTreeMap<String, BTreeMap<String, Vec<u8>>>,
    conflicts: BTreeMap<String, BTreeSet<String>>,
    edges: EntityEdges,
}
impl Collector {
    pub(crate) fn new(cursor: &str, version: u8) -> Self {
        Self {
            cursor: cursor.into(),
            version,
            groups: Vec::new(),
            index: BTreeMap::new(),
            facts: BTreeMap::new(),
            conflicts: BTreeMap::new(),
            edges: BTreeMap::new(),
        }
    }
    pub(crate) fn feed(&mut self, i: usize, r: &SearchResult) {
        let version = self.version;
        let cursor = self.cursor.as_str();
        let groups = &mut self.groups;
        let index = &mut self.index;
        let facts = &mut self.facts;
        let conflicts = &mut self.conflicts;
        let edges = &mut self.edges;
        let (identities, reason, data) = if version == 1 { v1(r) } else { v2(r) };
        let rid = format!("{}:{i}", cursor);
        if identities.is_empty() {
            groups.push(json!({"entity_id":null,"namespace":null,"identifier":null,"reason":reason,"result_ids":[rid],"conflicting_fields":[]}));
            return;
        }
        let mut ids = Vec::new();
        for (namespace, identifier) in identities {
            let encoded = canonical_json(
                &json!([version, namespace, identifier]),
                JsonProfile::AsciiCompact,
            )
            .expect("identity bytes");
            let id = format!(
                "entity-v{version}-{}",
                Sha256::digest(&encoded)
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect::<String>()
            );
            ids.push((namespace.clone(), id.clone()));
            let n=*index.entry(id.clone()).or_insert_with(||{let n=groups.len();groups.push(json!({"entity_id":id,"namespace":namespace,"identifier":identifier,"reason":"explicit_adapter_identifier","result_ids":[],"conflicting_fields":[]}));n});
            groups[n]["result_ids"]
                .as_array_mut()
                .expect("members")
                .push(json!(rid));
            for (key, value) in data.as_object().into_iter().flatten() {
                let identity = if version == 1 {
                    if namespace == "cve" {
                        key == "cve_id"
                    } else {
                        ["name", "version", "ecosystem"].contains(&key.as_str())
                    }
                } else {
                    [
                        "cve_id",
                        "name",
                        "version",
                        "ecosystem",
                        "doi",
                        "pmid",
                        "pmcid",
                        "openalex_id",
                        "repository",
                        "repository_url",
                        "ghsa_id",
                        "advisory_id",
                    ]
                    .contains(&key.as_str())
                };
                if identity {
                    continue;
                }
                let encoded = canonical_json(value, JsonProfile::AsciiCompact).expect("fact bytes");
                let old = facts
                    .entry(id.clone())
                    .or_default()
                    .entry(key.clone())
                    .or_insert_with(|| encoded.clone());
                if *old != encoded {
                    conflicts.entry(id.clone()).or_default().insert(key.clone());
                }
            }
        }
        if version == 2 {
            let package: Vec<_> = ids
                .iter()
                .filter(|(ns, _)| ["npm", "pypi"].contains(&ns.as_str()))
                .collect();
            let repos: Vec<_> = ids
                .iter()
                .filter(|(ns, _)| ns == "github_repository")
                .collect();
            let cves: Vec<_> = ids.iter().filter(|(ns, _)| ns == "cve").collect();
            let advisories: Vec<_> = ids
                .iter()
                .filter(|(ns, _)| ["ghsa", "osv_advisory"].contains(&ns.as_str()))
                .collect();
            let scholarly: Vec<_> = ids
                .iter()
                .filter(|(ns, _)| ["doi", "pmid", "pmcid", "openalex"].contains(&ns.as_str()))
                .collect();
            for (from, to, relation) in [
                (package.as_slice(), repos.as_slice(), "candidate_repository"),
                (
                    cves.as_slice(),
                    advisories.as_slice(),
                    "source_reported_advisory",
                ),
                (
                    scholarly.get(..1).unwrap_or(&[]),
                    scholarly.get(1..).unwrap_or(&[]),
                    "source_reported_alias",
                ),
            ] {
                for a in from {
                    for b in to {
                        let edge=edges.entry((a.1.clone(),relation.into(),b.1.clone())).or_insert_with(||json!({"from_entity_id":a.1,"relation":relation,"to_entity_id":b.1,"result_ids":[]}));
                        let members = edge["result_ids"].as_array_mut().expect("edge");
                        if !members.contains(&json!(rid)) {
                            members.push(json!(rid));
                        }
                    }
                }
            }
        }
    }
    fn finish(mut self) -> (Vec<Value>, EntityEdges) {
        for (id, n) in self.index {
            self.groups[n]["conflicting_fields"] =
                json!(self.conflicts.remove(&id).unwrap_or_default());
        }
        (self.groups, self.edges)
    }
    pub(crate) fn lineage_groups(self) -> Vec<ssx_application::lineage::resolver::EntityGroup> {
        self.finish()
            .0
            .into_iter()
            .filter_map(|g| {
                Some(ssx_application::lineage::resolver::EntityGroup {
                    id: g["entity_id"].as_str()?.into(),
                    namespace: g["namespace"].as_str()?.into(),
                    result_indices: g["result_ids"]
                        .as_array()?
                        .iter()
                        .map(|v| {
                            v.as_str()
                                .expect("generated result id")
                                .rsplit_once(':')
                                .expect("generated result index")
                                .1
                                .parse()
                                .expect("generated usize")
                        })
                        .collect(),
                })
            })
            .collect()
    }
}
pub(super) fn read(s: &SearchSnapshot, page: i64, start: usize, size: usize, version: u8) -> Value {
    let mut collector = Collector::new(s.id().as_str(), version);
    for (i, r) in s.results().iter().enumerate() {
        collector.feed(i, r);
    }
    let (groups, edges) = collector.finish();
    let unresolved = groups.iter().filter(|v| v["entity_id"].is_null()).count();
    let total = groups.len();
    let selected: Vec<_> = groups
        .into_iter()
        .skip(start)
        .take(size)
        .map(|mut v| {
            v["artifact"] = v["entity_id"]
                .as_str()
                .map(|id| {
                    artifact(
                        "entity_group",
                        &ssx_domain::canonical::composite_artifact_id(&[s.id().as_str(), id])
                            .expect("composite artifact"),
                    )
                })
                .unwrap_or(Value::Null);
            v
        })
        .collect();
    let mut out = json!({"contract":"slopsearx.entity_groups","version":version,"cursor":s.id().as_str(),"query":s.query(),"page":page,"entities":selected,"meta":{"total_entities":total,"total_results":s.total(),"unresolved_entities":unresolved,"has_more":start.saturating_add(size)<total,"query_id":s.query_id().as_str(),"note":"Entity identity is source-reported, not independent corroboration or verification."}});
    if version == 2 {
        out["relationships"] = json!(edges.into_values().collect::<Vec<_>>());
    }
    out
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn frozen_source_entity_grammar_conserves_indices_facts_edges_and_unicode16() {
        assert_eq!(unicode_normalization::UNICODE_VERSION, (16, 0, 0));
        let corpus: Value = ssx_json::from_str(include_str!(
            "../../../../docs/rust-rewrite/t06/entities.json"
        ))
        .unwrap();
        assert_eq!(corpus["unicode_version"], "16.0.0");
        let mut failures = Vec::new();
        for row in corpus["cases"].as_array().unwrap() {
            let results: Vec<_> = row["results"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| ssx_domain::storage::decode_result(v.clone()).unwrap())
                .collect();
            let snapshot = SearchSnapshot::new(
                ssx_domain::SnapshotId::new("snap-test").unwrap(),
                "q".into(),
                ssx_domain::QueryId::new("q1").unwrap(),
                results,
                Default::default(),
                ssx_domain::TenantId::new("default").unwrap(),
                ssx_domain::WallClockTimestamp::new(1.0).unwrap(),
                None,
            )
            .unwrap();
            let version = row["version"].as_u64().unwrap() as u8;
            let mut actual = read(&snapshot, 1, 0, 10000, version);
            for group in actual["entities"].as_array_mut().unwrap() {
                group.as_object_mut().unwrap().shift_remove("artifact");
            }
            if actual["entities"] != row["entities"] {
                failures.push(format!(
                    "{} v{} actual={} expected={}",
                    row["case"], version, actual["entities"], row["entities"]
                ));
            }
            if version == 2 && actual["relationships"] != row["relationships"] {
                failures.push(format!(
                    "{} relationships actual={} expected={}",
                    row["case"], actual["relationships"], row["relationships"]
                ));
            }
        }
        assert!(
            failures.is_empty(),
            "{} entity source differences\n{}",
            failures.len(),
            failures
                .iter()
                .take(15)
                .cloned()
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
}
