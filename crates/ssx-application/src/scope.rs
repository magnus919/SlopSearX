//! Pure source selection. The public explanation is not dispatch authority.
use crate::filters::FilterDeclarations;
use ssx_domain::{EngineExclusion, RoutingTradeoff, ScopeDecision, SearchRequest};
use std::collections::{BTreeMap, BTreeSet};

/// Trusted composition-root projection, with no secret values or provider I/O.
#[derive(Debug, Clone)]
pub struct EngineCapability {
    pub name: String,
    pub enabled: bool,
    pub active: bool,
    pub available: bool,
    pub categories: Vec<String>,
    pub media_types: Vec<String>,
    pub auth_ready: bool,
    pub circuit_open: bool,
    pub cost_class: String,
    pub health_status: String,
    pub health_stale: bool,
    pub enforced_filters: BTreeMap<String, String>,
}
#[derive(Debug, Clone)]
pub struct CatalogSnapshot {
    engines: Vec<EngineCapability>,
}
impl CatalogSnapshot {
    pub fn new(engines: Vec<EngineCapability>) -> Result<Self, &'static str> {
        let mut seen = BTreeSet::new();
        for e in &engines {
            if e.name.is_empty() || e.name.contains(char::is_whitespace) || !seen.insert(&e.name) {
                return Err("empty, invalid or duplicate catalog engine");
            }
        }
        Ok(Self { engines })
    }
    pub fn engines(&self) -> &[EngineCapability] {
        &self.engines
    }
    pub fn get(&self, name: &str) -> Option<&EngineCapability> {
        self.engines.iter().find(|e| e.name == name)
    }
    pub fn declarations(&self) -> FilterDeclarations {
        self.engines
            .iter()
            .filter(|e| e.active && e.enabled)
            .map(|e| (e.name.clone(), e.enforced_filters.clone()))
            .collect()
    }
}
#[derive(Debug, Clone, Default)]
pub struct RoutingBudget {
    pub max_engines: usize,
    pub max_cost_class: String,
    pub coverage_target: usize,
}
#[derive(Debug, Clone)]
pub struct Topic {
    pub name: String,
    pub keywords: Vec<String>,
    pub engines: Vec<String>,
}
#[derive(Debug, Clone)]
pub struct ScopeResolver {
    pub topics: Option<Vec<Topic>>,
    pub tier1: BTreeSet<String>,
    pub sensitive: BTreeSet<String>,
    pub catalog_routing: bool,
    pub budget: Option<RoutingBudget>,
}
impl Default for ScopeResolver {
    fn default() -> Self {
        Self {
            topics: None,
            tier1: [
                "brave",
                "duckduckgo",
                "google",
                "wikipedia",
                "stackexchange",
                "reddit",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect(),
            sensitive: ["hibp", "dehashed"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
            catalog_routing: true,
            budget: None,
        }
    }
}
fn strip(s: &str) -> &str {
    s.trim_matches(|c: char| c.is_whitespace() || matches!(c, '\u{1c}'..='\u{1f}'))
}
fn exclusion(d: &mut ScopeDecision, name: &str, stage: &str, reason: impl Into<String>) {
    d.excluded_engines.push(EngineExclusion {
        engine: name.into(),
        stage: stage.into(),
        reason: reason.into(),
    });
}
fn cost(s: &str) -> u8 {
    match s {
        "free" => 1,
        "freemium" => 2,
        "paid" => 3,
        _ => 0,
    }
}
fn health(e: &EngineCapability) -> u8 {
    if e.health_stale {
        return 1;
    }
    match e.health_status.as_str() {
        "ok" => 0,
        "rate_limited" => 2,
        "blocked" => 3,
        "timeout" => 4,
        "error" => 5,
        "unavailable" => 6,
        _ => 1,
    }
}
impl ScopeResolver {
    /// Raw scope presence suppresses optional advice, even whitespace-only lists.
    pub fn allowed(request: &SearchRequest) -> bool {
        !request.engines.as_ref().is_some_and(|v| !v.is_empty())
            && !request.categories.as_ref().is_some_and(|v| !v.is_empty())
            && request.media_type.is_none()
    }
    /// Advice uses automatic eligibility, never explicit-scope bypasses. Descriptor
    /// availability is intentionally deferred to dispatch authorization by F09.
    pub fn advice_eligible(
        &self,
        request: &SearchRequest,
        catalog: &CatalogSnapshot,
        name: &str,
    ) -> bool {
        if !Self::allowed(request)
            || self.sensitive.contains(name)
            || catalog.get(name).is_none_or(|e| !e.enabled || !e.active)
        {
            return false;
        }
        let mut gate = self.clone();
        gate.catalog_routing = true;
        if let Some(b) = &mut gate.budget {
            b.max_engines = 0;
            b.coverage_target = 0;
        }
        !gate
            .route(&mut ScopeDecision::default(), vec![name.into()], catalog)
            .is_empty()
    }
    /// Recompute the deterministic scope under current state, then admit optional
    /// advice into remaining automatic budget slots. Never evict baseline engines.
    pub fn with_specialist_advice(
        &self,
        request: &SearchRequest,
        catalog: &CatalogSnapshot,
        advice: &[(String, ssx_domain::FiniteF64)],
    ) -> ScopeDecision {
        let mut d = self.resolve(request, catalog);
        if !Self::allowed(request) || advice.is_empty() {
            return d;
        }
        let mut proposed = Vec::new();
        let mut proposed_scores = BTreeMap::new();
        for (n, score) in advice {
            if d.selected_engines.contains(n) || proposed.contains(n) {
                continue;
            }
            if !(0.0..=1.0).contains(&score.get()) || !self.advice_eligible(request, catalog, n) {
                exclusion(
                    &mut d,
                    n,
                    "advice",
                    "specialist advice rejected by current automatic eligibility",
                );
                continue;
            }
            proposed_scores.insert(n.clone(), score.clone());
            proposed.push(n.clone());
        }
        let remaining = self
            .budget
            .as_ref()
            .filter(|b| b.max_engines > 0)
            .map(|b| b.max_engines.saturating_sub(d.selected_engines.len()));
        let mut gate = self.clone();
        gate.catalog_routing = true;
        if let Some(b) = &mut gate.budget {
            b.max_engines = 0;
            b.coverage_target = 0;
        }
        let proposed = gate.route(&mut d, proposed, catalog);
        for n in proposed {
            if remaining.is_some_and(|limit| d.jev_added_engines.len() >= limit) {
                exclusion(
                    &mut d,
                    &n,
                    "budget",
                    "specialist advice exceeds remaining automatic engine budget",
                );
                d.routing_budget_applied = true;
                continue;
            }
            // Previously excluded engines cannot carry a contradictory admission.
            d.excluded_engines.retain(|e| e.engine != n);
            d.jev_scores.insert(n.clone(), proposed_scores[&n].clone());
            d.jev_added_engines.push(n.clone());
            d.selected_engines.push(n);
        }
        if !d.jev_added_engines.is_empty() {
            d.routing_rule.push_str("+jev specialists");
        }
        // Explain the final union, not the earlier baseline or intermediate advice.
        d.routing_tradeoffs.clear();
        d.warnings.retain(|w| !w.starts_with("routing trade-off: "));
        let selected_count = d.selected_engines.len();
        self.explain(&mut d, selected_count);
        d
    }
    pub fn resolve(&self, request: &SearchRequest, catalog: &CatalogSnapshot) -> ScopeDecision {
        let active: Vec<_> = catalog
            .engines
            .iter()
            .filter(|e| e.active && e.enabled)
            .collect();
        let mut d = ScopeDecision::default();
        let explicit = request.engines.as_ref().is_some_and(|v| !v.is_empty());
        let mut selected: Vec<String>;
        if explicit {
            let names: Vec<_> = request
                .engines
                .as_ref()
                .unwrap()
                .iter()
                .map(|s| strip(s))
                .filter(|s| !s.is_empty())
                .collect();
            let unknown: Vec<_> = names
                .iter()
                .filter(|s| !active.iter().any(|e| e.name == **s))
                .copied()
                .collect();
            if !unknown.is_empty() {
                let mut sorted = unknown.clone();
                sorted.sort();
                d.warnings
                    .push(format!("unknown engines ignored: {}", sorted.join(", ")));
                for n in sorted {
                    exclusion(&mut d, n, "policy", "not an active engine");
                }
            }
            selected = names
                .into_iter()
                .filter(|s| active.iter().any(|e| e.name == *s))
                .map(str::to_owned)
                .collect();
            d.routing_rule = "explicit engine".into();
        } else {
            let cats: Vec<String> = request
                .categories
                .iter()
                .flatten()
                .map(|s| strip(s))
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .collect();
            if !cats.is_empty() {
                selected = active
                    .iter()
                    .filter(|e| e.categories.iter().any(|c| cats.contains(c)))
                    .map(|e| e.name.clone())
                    .collect();
                d.resolved_categories = cats;
                d.routing_rule = "explicit category".into();
            } else if matches!(request.media_type.as_deref(), Some("image" | "video")) {
                let media = request.media_type.as_deref().unwrap();
                selected = Vec::new();
                for e in &active {
                    if e.media_types.iter().any(|m| m == media) {
                        selected.push(e.name.clone());
                    } else {
                        exclusion(
                            &mut d,
                            &e.name,
                            "policy",
                            format!("does not advertise media type '{media}'"),
                        );
                    }
                }
                if selected.is_empty() {
                    d.warnings
                        .push(format!("no active engine advertises media type '{media}'"));
                }
                d.routing_rule = "media type".into();
            } else if let Some(topics) = &self.topics {
                let query = strip(&request.query.to_lowercase()).to_owned();
                let topic = topics.iter().find(|t| {
                    !t.engines.is_empty() && t.keywords.iter().any(|k| query.contains(k))
                });
                if let Some(t) = topic {
                    selected = t
                        .engines
                        .iter()
                        .filter(|n| active.iter().any(|e| &e.name == *n))
                        .cloned()
                        .collect();
                    d.matched_topic = Some(t.name.clone());
                    d.routing_rule = "topic match".into();
                } else {
                    selected = self
                        .tier1
                        .iter()
                        .filter(|n| active.iter().any(|e| &e.name == *n))
                        .cloned()
                        .collect();
                    if selected.is_empty() {
                        selected = active.iter().map(|e| e.name.clone()).collect();
                        d.routing_rule = "all active engines".into();
                    } else {
                        d.routing_rule = "tier-1 fallback".into();
                    }
                }
            } else {
                selected = active.iter().map(|e| e.name.clone()).collect();
                d.routing_rule = "all active engines".into();
            }
            selected.retain(|n| {
                if self.sensitive.contains(n) {
                    exclusion(&mut d, n, "policy", "sensitive engine excluded by policy");
                    d.warnings.push(format!(
                        "sensitive engine '{n}' requires an explicit engines list or a policy grant"
                    ));
                    false
                } else {
                    true
                }
            });
            selected = self.route(&mut d, selected, catalog);
        }
        if let Some(media) = &request.media_type {
            if !matches!(media.as_str(), "image" | "video") {
                d.warnings.push(format!("unknown media type '{media}'"));
                selected.clear();
            } else {
                let had = !selected.is_empty();
                selected.retain(|n| {
                    if catalog
                        .get(n)
                        .is_some_and(|e| e.media_types.contains(media))
                    {
                        true
                    } else {
                        exclusion(
                            &mut d,
                            n,
                            "policy",
                            format!("does not advertise media type '{media}'"),
                        );
                        false
                    }
                });
                if had && selected.is_empty() {
                    d.warnings.push(format!(
                        "no selected engine advertises media type '{media}'"
                    ));
                }
            }
        }
        d.selected_engines = selected;
        d
    }
    fn route(
        &self,
        d: &mut ScopeDecision,
        mut selected: Vec<String>,
        catalog: &CatalogSnapshot,
    ) -> Vec<String> {
        if !self.catalog_routing {
            d.routing_fallback = true;
            return selected;
        }
        if selected.is_empty() {
            return selected;
        }
        selected.retain(|n| {
            let Some(e) = catalog.get(n) else {
                return true;
            };
            if !e.auth_ready {
                exclusion(
                    d,
                    n,
                    "auth",
                    "engine requires credentials that are not configured",
                );
                false
            } else if e.circuit_open {
                exclusion(
                    d,
                    n,
                    "health",
                    "engine circuit is open; dispatch is skipped until it recovers",
                );
                false
            } else {
                true
            }
        });
        if let Some(b) = &self.budget {
            let max = match cost(&b.max_cost_class) {
                0 => 3,
                n => n,
            };
            selected.retain(|n| {
                let c = catalog.get(n).map_or("", |e| e.cost_class.as_str());
                if cost(c) > max {
                    exclusion(
                        d,
                        n,
                        "budget",
                        format!(
                            "cost_class '{}' exceeds the configured budget max '{}'",
                            if c.is_empty() { "unknown" } else { c },
                            b.max_cost_class
                        ),
                    );
                    d.routing_budget_applied = true;
                    false
                } else {
                    true
                }
            });
            if b.max_engines > 0 && selected.len() > b.max_engines {
                selected.sort_by_key(|n| {
                    let e = catalog.get(n);
                    (
                        u8::from(!self.tier1.contains(n)),
                        e.map_or(1, health),
                        e.map_or(4, |e| match cost(&e.cost_class) {
                            0 => 4,
                            c => c,
                        }),
                        n.clone(),
                    )
                });
                let dropped = selected.split_off(b.max_engines);
                for n in dropped {
                    exclusion(
                        d,
                        &n,
                        "budget",
                        format!(
                            "scope exceeds the configured per-intent engine budget ({})",
                            b.max_engines
                        ),
                    );
                }
                d.routing_budget_applied = true;
            }
        }
        self.explain(d, selected.len());
        selected
    }
    /// Final explanation only: never changes engine selection or advice metadata.
    fn explain(&self, d: &mut ScopeDecision, selected_count: usize) {
        if d.excluded_engines.iter().any(|e| e.stage == "budget") {
            d.routing_tradeoffs.push(RoutingTradeoff{kind:"cost".into(),detail:"coverage is bounded by the configured routing budget; engines were excluded for cost or count".into()});
        }
        if d.excluded_engines.iter().any(|e| e.stage == "health") {
            d.routing_tradeoffs.push(RoutingTradeoff {
                kind: "availability".into(),
                detail: "coverage reflects only healthy engines; degraded engines were excluded"
                    .into(),
            });
        }
        if let Some(b) = &self.budget
            && b.coverage_target > 0
            && selected_count < b.coverage_target
        {
            d.routing_tradeoffs.push(RoutingTradeoff {
                kind: "coverage".into(),
                detail: format!(
                    "selected {} engines below the coverage target of {}",
                    selected_count, b.coverage_target
                ),
            });
        }
        if !d.routing_tradeoffs.is_empty() {
            d.warnings.push(format!(
                "routing trade-off: {}",
                d.routing_tradeoffs
                    .iter()
                    .map(|t| format!("{} — {}", t.kind, t.detail))
                    .collect::<Vec<_>>()
                    .join("; ")
            ));
        }
    }
}
