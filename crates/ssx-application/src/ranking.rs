//! Deterministic pure fusion. Feed maps preserve caller insertion order and enforce unique engine names.
use crate::urls::normalise_url;
use ssx_domain::{FiniteF64, SearchResult};
use std::collections::{BTreeMap, BTreeSet};
pub type Feeds = ssx_json::Map<String, Vec<SearchResult>>;
pub fn ranking_explanation(strategy: &str) -> &'static str {
    if strategy == "reciprocal_rank_fusion" {
        "tier_then_reciprocal_rank_fusion_k60"
    } else {
        "tier_then_cross_engine_presence"
    }
}
/// Caller evidence is immutable; only the returned view receives scores and positions.
pub fn rank(feeds: &Feeds, strategy: &str, budgets: &BTreeMap<String, i64>) -> Vec<SearchResult> {
    let rrf = strategy == "reciprocal_rank_fusion";
    let mut ordered: Vec<_> = feeds.iter().collect();
    if rrf {
        ordered.sort_by(|a, b| a.0.cmp(b.0));
    }
    let mut seen: BTreeMap<String, usize> = BTreeMap::new();
    let mut rows: Vec<(String, SearchResult)> = Vec::new();
    for (engine, feed) in ordered {
        let budget = budgets.get(engine).copied().unwrap_or(0);
        let mut contributed = BTreeSet::new();
        for (position, result) in feed.iter().enumerate() {
            let key = normalise_url(&result.url);
            if rrf && !contributed.insert(key.clone()) {
                continue;
            }
            let contribution = if rrf {
                1.0 / (61 + position) as f64
            } else {
                1.0
            };
            if let Some(&index) = seen.get(&key) {
                let existing = &mut rows[index].1;
                existing.engines.insert(engine.clone());
                existing.score = FiniteF64::new(if rrf {
                    existing.score.get() + contribution
                } else {
                    existing.engines.len() as f64
                })
                .expect("bounded score");
                existing.tier = existing.tier.min(result.tier);
            } else if rrf || budget <= 0 || (position as i64) < budget {
                let mut row = result.clone();
                row.engine = engine.clone();
                row.engines = BTreeSet::from([engine.clone()]);
                row.score = FiniteF64::new(contribution).expect("bounded score");
                seen.insert(key.clone(), rows.len());
                rows.push((key, row));
            }
        }
    }
    rows.sort_by(|a, b| {
        a.1.tier
            .cmp(&b.1.tier)
            .then_with(|| b.1.score.get().total_cmp(&a.1.score.get()))
            .then_with(|| {
                if rrf {
                    a.0.cmp(&b.0)
                } else {
                    std::cmp::Ordering::Equal
                }
            })
    });
    rows.into_iter()
        .enumerate()
        .map(|(i, (_, mut r))| {
            r.position = (i + 1) as i64;
            r
        })
        .collect()
}
