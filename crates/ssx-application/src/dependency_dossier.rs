//! Bounded dependency dossiers. Registry attribution is evidence, never ownership
//! or affected-version proof. Execution remains the existing research worker's job.
use crate::composition::{Error, ResolvedSource};
use sha2::{Digest, Sha256};
use ssx_domain::{
    SearchResult, TenantId, WallClockTimestamp,
    canonical::{JsonProfile, canonical_json},
    research::{ResearchCeilings, ResearchJob, ResearchQuery},
};
use ssx_json::{Value, json};
use std::collections::BTreeSet;

pub const CONTRACT: &str = "slopsearx.dependency_dossier";
pub const VERSION: u64 = 1;
fn invalid(message: &str) -> Error {
    Error::new("invalid_input", message)
}
fn segment(value: &str, max: usize, middle: fn(u8) -> bool) -> bool {
    !value.is_empty()
        && value.len() <= max
        && value.is_ascii()
        && value.as_bytes()[0].is_ascii_alphanumeric()
        && value.as_bytes()[value.len() - 1].is_ascii_alphanumeric()
        && value.bytes().all(middle)
}
pub fn normalize_package(ecosystem: &str, raw: &str) -> Result<String, Error> {
    let value = raw.trim_matches(ssx_json::python_whitespace);
    match ecosystem {
        "pypi"
            if segment(value, 200, |b| {
                b.is_ascii_alphanumeric() || b"._-".contains(&b)
            }) =>
        {
            let mut result = String::new();
            for b in value.bytes() {
                if b"._-".contains(&b) {
                    if !result.ends_with('-') {
                        result.push('-');
                    }
                } else {
                    result.push((b as char).to_ascii_lowercase());
                }
            }
            Ok(result)
        }
        "npm" => {
            let valid = |s: &str| {
                !s.is_empty()
                    && s.len() <= 214
                    && (s.as_bytes()[0].is_ascii_lowercase() || s.as_bytes()[0].is_ascii_digit())
                    && s.bytes().all(|b| {
                        b.is_ascii_lowercase() || b.is_ascii_digit() || b"._~-".contains(&b)
                    })
            };
            let parts: Vec<_> = value
                .strip_prefix('@')
                .unwrap_or(value)
                .split('/')
                .collect();
            if parts.len() == if value.starts_with('@') { 2 } else { 1 }
                && parts.iter().all(|s| valid(s))
            {
                Ok(value.into())
            } else {
                Err(invalid("invalid npm package name"))
            }
        }
        "pypi" => Err(invalid("invalid PyPI package name")),
        _ => Err(invalid("unsupported ecosystem")),
    }
}
pub fn normalize_repository(raw: Option<&str>) -> Result<Option<String>, Error> {
    let Some(raw) = raw else { return Ok(None) };
    let value = raw.trim_matches(ssx_json::python_whitespace);
    let path;
    let value = if value.contains("://") {
        // Reuse the accepted structural urllib-compatible splitter. A WHATWG
        // parser could fold a Unicode/percent-encoded host into github.com.
        let parsed = crate::urls::parse(value)
            .ok_or_else(|| invalid("repository must be a public canonical GitHub URL"))?;
        let credentials = parsed
            .authority
            .rsplit_once('@')
            .map(|(user, _)| user)
            .unwrap_or("");
        let (username, password) = credentials.split_once(':').unwrap_or((credentials, ""));
        let port_allowed = match parsed.port.as_deref() {
            None | Some("") => true,
            Some(p) => p.bytes().all(|b| b.is_ascii_digit()) && p.parse::<u16>().ok() == Some(0),
        };
        if parsed.scheme != "https"
            || !matches!(parsed.host.as_str(), "github.com" | "www.github.com")
            || !username.is_empty()
            || !password.is_empty()
            || !port_allowed
            || !parsed.query.is_empty()
            || !parsed.fragment.is_empty()
            || value.contains(';')
        {
            return Err(invalid("repository must be a public canonical GitHub URL"));
        }
        path = parsed.path.trim_matches('/').to_owned();
        path.as_str()
    } else {
        value
    };
    let value = value.strip_suffix(".git").unwrap_or(value);
    let parts: Vec<_> = value.split('/').collect();
    if parts.len() != 2
        || !parts
            .iter()
            .all(|s| segment(s, 100, |b| b.is_ascii_alphanumeric() || b"_.-".contains(&b)))
    {
        return Err(invalid("repository must be owner/repository"));
    }
    Ok(Some(value.into()))
}
pub fn github_root_candidate(raw: &str) -> Option<String> {
    let parsed = crate::urls::parse(raw.trim())?;
    if parsed.scheme != "https" || !matches!(parsed.host.as_str(), "github.com" | "www.github.com")
    {
        return None;
    }
    normalize_repository(Some(raw)).ok().flatten()
}
#[derive(Clone, Debug, PartialEq)]
pub struct Identity {
    pub value: Value,
    pub digest: String,
}
impl Identity {
    pub fn new(
        ecosystem: &str,
        package: &str,
        version: Option<&str>,
        repository: Option<&str>,
        deadline: Option<&str>,
    ) -> Result<Self, Error> {
        if version.is_some_and(|v| {
            v.trim_matches(ssx_json::python_whitespace).is_empty()
                || v.trim_matches(ssx_json::python_whitespace).chars().count() > 128
        }) {
            return Err(invalid("version must contain 1 to 128 characters"));
        }
        let value = json!({"ecosystem":ecosystem,"package":normalize_package(ecosystem,package)?,"requested_version":version.map(|s|s.trim_matches(ssx_json::python_whitespace)),"repository":normalize_repository(repository)?,"requested_deadline":deadline});
        let bytes = canonical_json(&value, JsonProfile::AsciiCompact)
            .map_err(|_| invalid("identity cannot be serialized"))?;
        let digest = Sha256::digest(bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        Ok(Self { value, digest })
    }
}
fn package_data<'a>(result: &'a SearchResult, ecosystem: &str) -> Option<&'a Value> {
    let payload = result.payload.as_ref()?;
    let provenance = payload.get("provenance")?;
    if payload["domain"] != "packages"
        || payload["type"] != "package"
        || payload["schema_version"] != 1
        || result.engine != ecosystem
        || provenance["engine"] != ecosystem
        || !provenance["adapter_fields"]
            .as_array()?
            .iter()
            .any(|v| v.as_str() == Some("name"))
        || provenance["inferred_fields"]
            .as_array()
            .is_some_and(|a| a.iter().any(|v| v.as_str() == Some("name")))
    {
        return None;
    }
    payload.get("data").filter(|v| v.is_object())
}
pub fn resolve_package_results(
    ecosystem: &str,
    requested: &str,
    results: &[SearchResult],
) -> Value {
    let mut exact = vec![];
    let mut candidates = vec![];
    let mut repositories = BTreeSet::new();
    let mut distinct = BTreeSet::new();
    for (index, result) in results.iter().enumerate() {
        let Some(data) = package_data(result, ecosystem) else {
            continue;
        };
        let Some(name) = data["name"].as_str() else {
            continue;
        };
        let normalized = normalize_package(ecosystem, name).unwrap_or_default();
        let version = data["version"].as_str();
        let item =
            json!({"index":index,"name":name,"normalized_name":normalized,"version":version});
        candidates.push(item.clone());
        if normalized == requested {
            exact.push(item);
            distinct.insert((normalized, version.map(str::to_owned)));
            for field in ["repository_url", "homepage"] {
                if let Some(candidate) = data[field].as_str().and_then(github_root_candidate) {
                    repositories.insert(candidate);
                }
            }
        }
    }
    json!({"status":if exact.is_empty(){"unresolved"}else if distinct.len()>1{"ambiguous"}else{"resolved"},"basis":if exact.is_empty(){None}else{Some("attributed_registry_payload")},"matches":exact,"candidates":candidates,"repository_candidates":repositories.into_iter().collect::<Vec<_>>()})
}
#[derive(Clone, Debug)]
pub struct Limits {
    pub queries: u64,
    pub results: u64,
    pub max_results: usize,
}
impl Limits {
    pub fn validate(&self) -> Result<(), Error> {
        if self.queries == 0
            || self.queries > 100
            || self.results == 0
            || self.max_results == 0
            || self.max_results > 1000
        {
            Err(invalid("invalid dossier limits"))
        } else {
            Ok(())
        }
    }
}
/// Pure admission candidate. Callers must authorize the entire plan before create.
#[allow(clippy::too_many_arguments)]
pub fn plan(
    identity: Identity,
    tenant: &TenantId,
    job_id: &str,
    now: WallClockTimestamp,
    deadline: WallClockTimestamp,
    idempotency: Option<&str>,
    source: Option<&ResolvedSource>,
    limits: &Limits,
) -> Result<ResearchJob, Error> {
    limits.validate()?;
    if deadline.seconds() <= now.seconds() {
        return Err(invalid("deadline must be in the future"));
    }
    if idempotency.is_some_and(|v| {
        v.trim_matches(ssx_json::python_whitespace).is_empty()
            || v.trim_matches(ssx_json::python_whitespace).chars().count() > 128
    }) {
        return Err(invalid("idempotency_key must contain 1 to 128 characters"));
    }
    let ecosystem = identity.value["ecosystem"]
        .as_str()
        .ok_or_else(|| invalid("missing ecosystem"))?;
    let package = identity.value["package"]
        .as_str()
        .ok_or_else(|| invalid("missing package"))?;
    let mut queries = vec![ResearchQuery {
        query: package.into(),
        intent: "packages".into(),
        engines: vec![ecosystem.into()],
        ..Default::default()
    }];
    let repository_index = if let Some(repo) = identity.value["repository"].as_str() {
        queries.push(ResearchQuery {
            index: 1,
            query: format!("repo:{repo}"),
            intent: "code".into(),
            engines: vec!["github".into()],
            ..Default::default()
        });
        Some(1usize)
    } else {
        None
    };
    let advisory_index = queries.len();
    let advisory = format!(
        "{ecosystem} {package}{}",
        identity.value["requested_version"]
            .as_str()
            .map(|v| format!(" {v}"))
            .unwrap_or_default()
    );
    queries.push(ResearchQuery {
        index: advisory_index as u64,
        query: advisory,
        intent: "security".into(),
        engines: vec!["nvd".into()],
        ..Default::default()
    });
    if queries.len() as u64 > limits.queries {
        return Err(Error::new(
            "budget_exceeded",
            "research query budget cannot fund required dossier sections",
        ));
    }
    let mut workflow = json!({"kind":"dependency_dossier","version":VERSION,"identity":identity.value,"identity_digest":identity.digest,"section_queries":{"package_information":0,"repository_records":repository_index,"advisory_leads":advisory_index},"budget":{"max_adapter_calls":limits.queries.min(queries.len() as u64*2),"max_results":limits.results,"used_adapter_calls":0,"captured_results":0,"admitted_results":{}}});
    if let Some(source) = source {
        workflow["composition"] = json!({"source":source.stored()});
        workflow["lineage"] = json!([{"from":ssx_domain::ArtifactRef::new(ssx_domain::ArtifactKind::DependencyDossier,job_id).map_err(|_|invalid("invalid dossier id"))?.to_value(),"relation":"derived_from","to":source.artifact.to_value()}]);
    }
    let mut job = ResearchJob {
        job_id: job_id.into(),
        question: format!("Dependency dossier for {ecosystem}:{package}"),
        strategy: "dependency_dossier".into(),
        queries,
        created_at: ssx_domain::FiniteF64::new(now.seconds())
            .map_err(|_| invalid("invalid creation time"))?,
        deadline: ssx_domain::FiniteF64::new(deadline.seconds())
            .map_err(|_| invalid("invalid deadline"))?,
        tenant: tenant.as_str().into(),
        idempotency_key: idempotency.map(|v| v.trim_matches(ssx_json::python_whitespace).into()),
        workflow: workflow
            .as_object()
            .ok_or_else(|| invalid("invalid workflow"))?
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect(),
        ..Default::default()
    };
    job.initialize_budget(ResearchCeilings {
        queries: limits.queries,
        engines_per_query: 1,
        results: limits.results,
    })
    .map_err(|e| invalid(e.0))?;
    job.validate().map_err(|e| invalid(e.0))?;
    Ok(job)
}
pub fn same_request(existing: &ResearchJob, candidate: &ResearchJob) -> bool {
    existing.workflow.get("kind") == Some(&json!("dependency_dossier"))
        && existing.workflow.get("identity_digest") == candidate.workflow.get("identity_digest")
        && existing
            .workflow
            .get("composition")
            .and_then(|c| c.get("source"))
            .and_then(|s| s.get("artifact"))
            == candidate
                .workflow
                .get("composition")
                .and_then(|c| c.get("source"))
                .and_then(|s| s.get("artifact"))
}
pub fn start_envelope(job: &ResearchJob, replay: bool) -> Value {
    let mut value = json!({"contract":CONTRACT,"version":VERSION,"job_id":job.job_id,"artifact":ssx_domain::ArtifactRef::new(ssx_domain::ArtifactKind::DependencyDossier, &job.job_id).expect("validated job id").to_value(),"state":job.state,"requested_identity":job.workflow.get("identity"),"created_at":job.created_at.get(),"deadline":job.deadline.get(),"replay":replay});
    if let Some(composition) = job.workflow.get("composition") {
        value["source"] = composition["source"].clone();
        value["lineage"] = job.workflow.get("lineage").cloned().unwrap_or(json!([]));
    }
    value
}

/// Complete retained section and unsliced admitted evidence. `value.results` is
/// presentation only; attribution always uses all admitted `results` below.
pub struct Section {
    pub value: Value,
    pub results: Vec<SearchResult>,
}
fn errors(query: &ResearchQuery) -> Vec<String> {
    query
        .engine_coverage
        .iter()
        .filter_map(|c| c.failure_class.as_deref())
        .filter(|s| *s != "ok")
        .map(|s| {
            match s {
                "error" => "upstream_error",
                other => other,
            }
            .into()
        })
        .collect()
}
pub fn section_without_snapshot(query: Option<&ResearchQuery>) -> Section {
    let Some(query) = query else {
        return Section {
            value: json!({"state":"unresolved","error":"not_executed","query":null,"query_id":null,"cursor":null,"executed_at":null,"source_published_at":[],"coverage":[],"results":[]}),
            results: vec![],
        };
    };
    let errors = errors(query);
    let (state, error) = match query.state.as_str() {
        "pending" | "running" => ("partial", Some("not_executed".to_owned())),
        "failed" => (
            if query
                .engine_coverage
                .iter()
                .any(|c| c.bucket == "unavailable")
            {
                "unavailable"
            } else {
                "failed"
            },
            Some(errors.first().cloned().unwrap_or("upstream_error".into())),
        ),
        _ => (
            if query.result_count == 0 {
                "empty"
            } else {
                "failed"
            },
            None,
        ),
    };
    Section {
        value: json!({"query":query.query,"query_id":query.query_id,"cursor":query.cursor,"executed_at":query.attempts.last().map(|a|a.attempted_at.get()),"coverage":query.engine_coverage,"errors":errors,"results":[],"source_published_at":[],"state":state,"error":error}),
        results: vec![],
    }
}
/// Shared native result projection can be supplied by final transports. This
/// application service never depends on the MCP registry or protocol stack.
pub fn section_from_snapshot(
    job: &ResearchJob,
    index: usize,
    snapshot: &ssx_domain::SearchSnapshot,
    limit: usize,
    repository: Option<&str>,
    card: &dyn Fn(&SearchResult, &str) -> Value,
) -> Result<Section, Error> {
    let query = job
        .queries
        .get(index)
        .ok_or_else(|| invalid("invalid section query"))?;
    if query.cursor.as_deref() != Some(snapshot.id().as_str())
        || job.tenant != snapshot.tenant().as_str()
    {
        return Err(Error::new(
            "source_malformed",
            "section snapshot identity differs from the job",
        ));
    }
    let legacy = job
        .workflow
        .get("budget")
        .and_then(|v| v.get("admitted_results"))
        .and_then(|v| v.get(&index.to_string()))
        .and_then(Value::as_u64);
    let native = query
        .attempts
        .iter()
        .rev()
        .find(|a| a.cursor == query.cursor && !a.attempt_id.is_empty());
    let admitted = |i: usize| {
        if let Some(attempt) = native {
            attempt
                .admitted_result_ids
                .iter()
                .any(|id| id == &format!("{}:{i}", snapshot.id()))
        } else {
            legacy.is_none_or(|n| i < (n as usize))
        }
    };
    let indexed = snapshot
        .results()
        .iter()
        .enumerate()
        .filter(|(i, r)| {
            admitted(*i)
                && repository
                    .is_none_or(|repo| github_root_candidate(&r.url).as_deref() == Some(repo))
        })
        .collect::<Vec<_>>();
    let mut section = section_without_snapshot(Some(query));
    section.value["state"] = json!(if indexed.is_empty() {
        "empty"
    } else {
        "available"
    });
    section.value["error"] = Value::Null;
    section.value["results"] = json!(
        indexed
            .iter()
            .take(limit)
            .map(|(i, r)| card(r, &format!("{}:{i}", snapshot.id())))
            .collect::<Vec<_>>()
    );
    section.value["source_published_at"] = json!(
        indexed
            .iter()
            .filter_map(|(_, r)| r.published_date.as_ref())
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
    );
    section.results = indexed.into_iter().map(|(_, r)| r.clone()).collect();
    Ok(section)
}
pub fn report(job: &ResearchJob, mut sections: [Section; 3]) -> Result<Value, Error> {
    let identity = job
        .workflow
        .get("identity")
        .ok_or_else(|| invalid("missing dossier identity"))?;
    let ecosystem = identity["ecosystem"]
        .as_str()
        .ok_or_else(|| invalid("missing dossier ecosystem"))?;
    let package = identity["package"]
        .as_str()
        .ok_or_else(|| invalid("missing dossier package"))?;
    let mut resolution = resolve_package_results(ecosystem, package, &sections[0].results);
    let observed = resolution["matches"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|m| m["version"].as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    let version = identity["requested_version"].as_str();
    let version_match = if let Some(v) = version.filter(|_| !observed.is_empty()) {
        if observed.contains(v) {
            "exact"
        } else {
            "different"
        }
    } else {
        "unknown"
    };
    let repositories = resolution
        .as_object_mut()
        .ok_or_else(|| invalid("invalid resolution"))?
        .shift_remove("repository_candidates")
        .unwrap_or(json!([]));
    resolution["requested_version"] = identity["requested_version"].clone();
    resolution["observed_versions"] = json!(observed);
    resolution["version_match"] = json!(version_match);
    let candidates = repositories
        .as_array()
        .ok_or_else(|| invalid("invalid repository candidates"))?;
    let repository = if let Some(repo) = identity["repository"].as_str() {
        json!({"status":"requested","repository":repo,"basis":"caller_supplied","proven_package_ownership":false,"metadata_conflict":!candidates.is_empty() && repositories!=json!([repo]),"metadata_candidates":repositories})
    } else {
        json!({"status":if candidates.is_empty(){"unresolved"}else if candidates.len()==1{"unverified_metadata"}else{"ambiguous"},"repository":if candidates.len()==1{Some(candidates[0].clone())}else{None},"basis":if candidates.is_empty(){None}else{Some("attributed_registry_metadata")},"proven_package_ownership":false,"metadata_candidates":repositories})
    };
    sections[2].value["applicability"] = json!({"status":"not_evaluated","requested_version":identity["requested_version"],"reason":"search leads do not expose attributable affected-version ranges"});
    let partial = ["partial", "failed", "cancelled", "expired"].contains(&job.state.as_str())
        || sections.iter().any(|s| {
            matches!(
                s.value["state"].as_str(),
                Some("partial" | "failed" | "unavailable" | "expired")
            )
        });
    let names = [
        "package_information",
        "repository_records",
        "advisory_leads",
    ];
    let missing=sections.iter().zip(names).filter(|(s,_)|!matches!(s.value["state"].as_str(),Some("available"|"empty"))).map(|(s,name)|json!({"section":name,"state":s.value["state"],"errors":s.value.get("errors").cloned().unwrap_or(json!([]))})).collect::<Vec<_>>();
    let mut followups = vec![];
    if matches!(
        repository["status"].as_str(),
        Some("unresolved" | "ambiguous" | "unverified_metadata")
    ) {
        followups.push(format!(
            "Find the canonical repository published by {ecosystem} for {package}"
        ));
    }
    followups.push(format!(
        "Verify affected-version ranges for advisory leads concerning {package}"
    ));
    let mut result = json!({"contract":CONTRACT,"version":VERSION,"job_id":job.job_id,"artifact":ssx_domain::ArtifactRef::new(ssx_domain::ArtifactKind::DependencyDossier,&job.job_id).map_err(|_|invalid("invalid dossier id"))?.to_value(),"state":job.state,"partial":partial,"budget":job.workflow.get("budget"),"requested_identity":identity,"resolved_package_identity":resolution,"repository_identity":repository,"sections":{"package_information":sections[0].value,"repository_records":sections[1].value,"advisory_leads":sections[2].value},"missing_source_coverage":missing,"suggested_followup_searches":followups,"limitations":["advisory results are search leads, not confirmed affected-version findings","repository candidates do not prove package ownership","zero advisory leads does not establish absence of vulnerabilities"]});
    if let Some(composition) = job.workflow.get("composition") {
        result["source"] = composition["source"].clone();
        result["lineage"] = job.workflow.get("lineage").cloned().unwrap_or(json!([]));
    }
    Ok(result)
}
