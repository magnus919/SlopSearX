//! Bounded identifier/version graph. Titles and dates never establish identity.
use crate::{
    ranking::Feeds,
    urls::{normalise_url, parse},
};
use ssx_domain::{
    SearchResult,
    canonical::{self, JsonProfile},
};
use ssx_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
pub const POLICY_VERSION: &str = "scholarly-work-v3";
pub const MAX_MEMBERS: usize = 64;
pub const MAX_GROUP_BYTES: usize = 64_000;
type Ids = BTreeMap<String, String>;
fn data(r: &SearchResult) -> Value {
    r.payload
        .as_ref()
        .filter(|p| {
            p["domain"] == json!("science")
                && p["type"] == json!("publication")
                && p["data"].is_object()
        })
        .map(|p| p["data"].clone())
        .unwrap_or_else(|| json!({}))
}
fn digits(s: &str) -> bool {
    !s.is_empty() && s.chars().all(ssx_json::decimal_digit)
}
fn letter(c: char) -> bool {
    c.is_ascii_alphabetic() || ['İ', 'ı', 'ſ', 'K'].contains(&c)
}
fn arxiv(s: &str) -> Option<(String, Option<u32>)> {
    let (base, version) = if let Some((base, v)) =
        s.rsplit_once(['v', 'V']).filter(|(_, v)| digits(v))
    {
        if v.chars().count() > 6 || !v.starts_with(|c: char| ('1'..='9').contains(&c)) || !digits(v)
        {
            return None;
        }
        // Python decimal int accepts Unicode decimal digits; convert using the shared digit table.
        let value = v
            .chars()
            .try_fold(0u32, |a, c| decimal(c).map(|n| a * 10 + n))?;
        (base, Some(value))
    } else {
        (s, None)
    };
    let valid = if let Some((subject, num)) = base.split_once('/') {
        let (name, area) = subject
            .split_once('.')
            .map_or((subject, None), |(a, b)| (a, Some(b)));
        !name.is_empty()
            && name.chars().all(|c| letter(c) || c == '-')
            && area.is_none_or(|a| a.chars().count() == 2 && a.chars().all(letter))
            && num.chars().count() == 7
            && digits(num)
    } else if let Some((year, num)) = base.split_once('.') {
        year.chars().count() == 4
            && digits(year)
            && (4..=5).contains(&num.chars().count())
            && digits(num)
    } else {
        false
    };
    valid.then(|| (ssx_json::casefold(base), version))
}
fn decimal(c: char) -> Option<u32> {
    if !ssx_json::decimal_digit(c) {
        return None;
    }
    // Every Unicode Nd block consists of consecutive runs of ten digits.
    let n = c as u32;
    let mut start = n;
    while start > 0 && char::from_u32(start - 1).is_some_and(ssx_json::decimal_digit) {
        start -= 1;
    }
    Some((n - start) % 10)
}
fn url_ids(raw: &str) -> Ids {
    let Some(p) = parse(raw) else {
        return Ids::new();
    };
    let path = p.path.trim_matches('/');
    let host = ssx_json::casefold(&p.host);
    let mut ids = Ids::new();
    if ["doi.org", "dx.doi.org"].contains(&host.as_str()) {
        if let Some(d) = ssx_publication::normalize_doi(path) {
            ids.insert("doi".into(), d);
        }
    } else if ["nature.com", "www.nature.com"].contains(&host.as_str()) {
        if path
            .chars()
            .take(9)
            .map(crate::text::ignorecase)
            .collect::<String>()
            == "articles/"
        {
            let prefix_end = path.char_indices().nth(9).map_or(path.len(), |(i, _)| i);
            let t = &path[prefix_end..];
            let chunks: Vec<_> = t.split('-').collect();
            if chunks.len() == 4
                && chunks[0].starts_with(['s', 'S', 'ſ'])
                && chunks[0].chars().skip(1).count() == 5
                && digits(&chunks[0].chars().skip(1).collect::<String>())
                && chunks[1].chars().count() == 3
                && digits(chunks[1])
                && (4..=5).contains(&chunks[2].chars().count())
                && digits(chunks[2])
                && chunks[3].chars().count() == 1
                && chunks[3].chars().all(|c| letter(c) || c.is_ascii_digit())
            {
                ids.insert("doi".into(), format!("10.1038/{}", ssx_json::casefold(t)));
            }
        }
    } else if host == "pubmed.ncbi.nlm.nih.gov"
        && !path.is_empty()
        && path.chars().all(crate::text::digit)
    {
        ids.insert("pmid".into(), path.into());
    } else if ["pmc.ncbi.nlm.nih.gov", "www.ncbi.nlm.nih.gov"].contains(&host.as_str()) {
        // The complete source pattern (optional pmc/, articles/, PMC) uses
        // re.IGNORECASE, including Python's dotted/dotless I and long S.
        let folded: String = path.chars().map(crate::text::ignorecase).collect();
        let path = folded.strip_prefix("pmc/").unwrap_or(&folded);
        if let Some(id) = path.strip_prefix("articles/pmc")
            && digits(id)
        {
            ids.insert("pmcid".into(), format!("PMC{id}"));
        }
    } else if ["arxiv.org", "www.arxiv.org", "export.arxiv.org"].contains(&host.as_str()) {
        let id = if let Some(id) = path.strip_prefix("html/") {
            id
        } else {
            path.strip_prefix("abs/")
                .or_else(|| path.strip_prefix("pdf/"))
                .unwrap_or(path)
                .strip_suffix(".pdf")
                .unwrap_or(
                    path.strip_prefix("abs/")
                        .or_else(|| path.strip_prefix("pdf/"))
                        .unwrap_or(path),
                )
        };
        if arxiv(id).is_some() {
            ids.insert("arxiv".into(), ssx_json::casefold(id));
        }
    }
    ids
}
pub fn publication_ids(r: &SearchResult) -> Ids {
    let mut ids = url_ids(&r.url);
    let d = data(r);
    if let Some(doi) = d["doi"].as_str().and_then(ssx_publication::normalize_doi) {
        if ids.get("doi").is_some_and(|v| v != &doi) {
            return Ids::new();
        }
        ids.insert("doi".into(), doi);
    }
    for key in ["pmid", "pmcid"] {
        if let Some(raw) = d[key].as_str() {
            let suffix =
                if key == "pmcid" && raw.get(..3).is_some_and(|s| s.eq_ignore_ascii_case("pmc")) {
                    &raw[3..]
                } else {
                    raw
                };
            if digits(suffix) {
                let value = if key == "pmcid" {
                    format!("PMC{suffix}")
                } else {
                    raw.into()
                };
                if ids.get(key).is_some_and(|v| v != &value) {
                    return Ids::new();
                }
                ids.insert(key.into(), value);
            }
        }
    }
    let raw = d
        .get("arxiv_id")
        .filter(|v| truthy(v))
        .or_else(|| {
            if r.engine == "arxiv" {
                d.get("publication_id")
            } else {
                None
            }
        })
        .and_then(Value::as_str);
    if let Some(raw) = raw
        && arxiv(raw).is_some()
    {
        let value = ssx_json::casefold(raw);
        if ids.get("arxiv").is_some_and(|v| v != &value) {
            return Ids::new();
        }
        ids.insert("arxiv".into(), value);
    }
    ids
}
pub fn paper_fields(r: &SearchResult) -> BTreeMap<String, Value> {
    let mut d = data(r);
    if let Some(doi) = publication_ids(r).get("doi") {
        d["doi"] = json!(doi);
    } else if let Some(o) = d.as_object_mut() {
        o.shift_remove("doi");
    }
    ssx_publication::paper_fields(Some(&json!({"paper":d})))
}
fn bounded_payload(r: &SearchResult) -> Option<Value> {
    r.payload
        .as_ref()
        .filter(|p| p.is_object())
        .filter(|p| size(p).is_some_and(|n| n <= MAX_GROUP_BYTES))
        .cloned()
}
fn size(v: &Value) -> Option<usize> {
    canonical::canonical_json(v, JsonProfile::Utf8Compact)
        .ok()
        .map(|b| b.len())
}
fn root(parent: &[usize], mut i: usize) -> usize {
    while parent[i] != i {
        i = parent[i];
    }
    i
}
struct Graph<'a> {
    entries: Vec<(&'a str, usize, &'a SearchResult)>,
    ids: Vec<Ids>,
    parent: Vec<usize>,
    members: Vec<Vec<usize>>,
}
impl Graph<'_> {
    fn record(&self, i: usize) -> Value {
        let (engine, pos, r) = self.entries[i];
        json!({"engine":engine,"original_position":pos,"url":r.url,"title":r.title,"content":r.content,"published_date":r.published_date,"identifiers":self.ids[i],"payload":bounded_payload(r)})
    }
    fn join(&mut self, left: usize, right: usize, version: bool) {
        let (l, r) = (root(&self.parent, left), root(&self.parent, right));
        if l == r || self.members[l].len() + self.members[r].len() > MAX_MEMBERS {
            return;
        }
        if !version {
            for key in ["doi", "pmid", "pmcid", "arxiv"] {
                let a: BTreeSet<_> = self.members[l]
                    .iter()
                    .filter_map(|&i| self.ids[i].get(key))
                    .collect();
                let b: BTreeSet<_> = self.members[r]
                    .iter()
                    .filter_map(|&i| self.ids[i].get(key))
                    .collect();
                if !a.is_empty() && !b.is_empty() && a != b {
                    return;
                }
            }
        }
        let indexes: Vec<_> = self.members[l]
            .iter()
            .chain(&self.members[r])
            .copied()
            .collect();
        if indexes.iter().any(|&i| {
            self.entries[i].2.payload.is_some() && bounded_payload(self.entries[i].2).is_none()
        }) {
            return;
        }
        let records: Vec<_> = indexes.iter().map(|&i| self.record(i)).collect();
        if size(&json!({"members":records})).is_none_or(|n| n > MAX_GROUP_BYTES) {
            return;
        }
        self.parent[r] = l;
        self.members[l] = indexes;
    }
}
pub fn group_publications(feeds: &Feeds, query: &str) -> Feeds {
    let entries: Vec<_> = feeds
        .iter()
        .flat_map(|(engine, rows)| {
            rows.iter()
                .enumerate()
                .map(move |(i, r)| (engine.as_str(), i + 1, r))
        })
        .collect();
    let n = entries.len();
    let ids = entries.iter().map(|(_, _, r)| publication_ids(r)).collect();
    let mut g = Graph {
        entries,
        ids,
        parent: (0..n).collect(),
        members: (0..n).map(|i| vec![i]).collect(),
    };
    let mut seen = BTreeMap::new();
    for i in 0..n {
        let keys = std::iter::once(format!("url:{}", normalise_url(&g.entries[i].2.url)))
            .chain(g.ids[i].iter().map(|(k, v)| format!("{k}:{v}")))
            .collect::<Vec<_>>();
        for key in keys {
            if let Some(&j) = seen.get(&key) {
                g.join(i, j, false);
            } else {
                seen.insert(key, i);
            }
        }
    }
    let mut newer = BTreeSet::new();
    let mut older = BTreeSet::new();
    for i in 0..n {
        if let Some((base, _)) = g.ids[i].get("arxiv").and_then(|s| arxiv(s)) {
            let key = format!("arxiv-base:{base}");
            if let Some(&j) = seen.get(&key) {
                g.join(i, j, true);
            } else {
                seen.insert(key, i);
            }
        }
        let d = data(g.entries[i].2);
        if let Some(relations) = d["relations"].as_array() {
            for relation in relations.iter().take(32) {
                if let (Some(kind), Some(target), Some(_)) = (
                    relation["relation_type"].as_str(),
                    relation["doi"]
                        .as_str()
                        .and_then(ssx_publication::normalize_doi),
                    g.ids[i].get("doi"),
                ) {
                    let kind = ssx_json::casefold(kind);
                    if [
                        "isversionof",
                        "hasversion",
                        "isnewversionof",
                        "ispreviousversionof",
                        "ispreprintof",
                        "haspreprint",
                    ]
                    .contains(&kind.as_str())
                        && let Some(&j) = seen.get(&format!("doi:{target}"))
                    {
                        g.join(i, j, true);
                        if kind == "isnewversionof" {
                            newer.insert(i);
                            older.insert(j);
                        } else if kind == "ispreviousversionof" {
                            newer.insert(j);
                            older.insert(i);
                        }
                    }
                }
            }
        }
    }
    // Iterate original positions, never union root order.
    let mut grouped: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for i in 0..n {
        grouped.entry(root(&g.parent, i)).or_default().push(i);
    }
    let mut reps = BTreeMap::new();
    for (key, indexes) in grouped {
        let mut candidate = indexes[0];
        let explicit: Vec<_> = indexes
            .iter()
            .copied()
            .filter(|i| newer.contains(i) && !older.contains(i))
            .collect();
        if explicit.len() == 1 {
            candidate = explicit[0];
        }
        let versions: Vec<_> = indexes
            .iter()
            .filter_map(|&i| {
                g.ids[i]
                    .get("arxiv")
                    .and_then(|s| arxiv(s))
                    .and_then(|(b, v)| v.map(|v| (b, v, i)))
            })
            .collect();
        let ordered = !versions.is_empty()
            && versions.len() == indexes.len()
            && versions.iter().all(|v| v.0 == versions[0].0)
            && explicit.is_empty();
        if ordered {
            candidate = versions
                .iter()
                .max_by(|a, b| a.1.cmp(&b.1).then_with(|| b.2.cmp(&a.2)))
                .expect("nonempty")
                .2;
        }
        let requested: Vec<_> = indexes
            .iter()
            .copied()
            .filter(|&i| {
                g.ids[i]
                    .get("arxiv")
                    .is_some_and(|id| id.contains('v') && query_contains(query, id))
            })
            .collect();
        let requested_revision = requested
            .iter()
            .filter_map(|i| g.ids[*i].get("arxiv"))
            .collect::<BTreeSet<_>>()
            .len()
            == 1;
        if requested_revision {
            candidate = requested[0];
        }
        let mut selected = g.entries[candidate].2.clone();
        let mut fields = paper_fields(&selected);
        for &i in &indexes {
            if !g.ids[i].is_empty() && g.ids[i] == g.ids[candidate] {
                for (k, v) in paper_fields(g.entries[i].2) {
                    fields.entry(k).or_insert(v);
                }
            }
        }
        let warnings: BTreeSet<_> = indexes
            .iter()
            .flat_map(|&i| {
                data(g.entries[i].2)["publication_types"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default()
            })
            .filter_map(|v| v.as_str().map(str::to_owned))
            .filter(|s| {
                [
                    "Retracted Publication",
                    "Retraction of Publication",
                    "Published Erratum",
                    "Corrected and Republished Article",
                    "Withdrawn Publication",
                ]
                .contains(&s.as_str())
            })
            .collect();
        if !warnings.is_empty() {
            let mut parts = Vec::new();
            if let Some(s) = fields
                .get("comments")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
            {
                parts.push(s.to_owned());
            }
            parts.extend(
                warnings
                    .iter()
                    .map(|w| format!("Source member notice: {w}")),
            );
            fields.insert("comments".into(), json!(parts.join("; ")));
        }
        selected.work_group = if indexes.iter().any(|&i| {
            !g.ids[i].is_empty()
                || data(g.entries[i].2)
                    .as_object()
                    .is_some_and(|o| !o.is_empty())
        }) {
            Some(
                json!({"schema_version":1,"paper":fields,"representative_engine":g.entries[candidate].0,"members":indexes.iter().map(|&i|g.record(i)).collect::<Vec<_>>(),"selection":if requested_revision{"requested_revision"}else if ordered||explicit.len()==1{"explicit_revision"}else{"stable_source_order"},"warnings":warnings}),
            )
        } else {
            None
        };
        reps.insert(key, selected);
    }
    let mut offset = 0;
    feeds
        .iter()
        .map(|(engine, rows)| {
            let mut output = Vec::new();
            let mut done = BTreeSet::new();
            for original in rows {
                let key = root(&g.parent, offset);
                offset += 1;
                let mut result = reps[&key].clone();
                if result.work_group.is_some() && !done.insert(key) {
                    continue;
                }
                result.engine = engine.clone();
                result.engines = BTreeSet::from([engine.clone()]);
                result.tier = original.tier;
                output.push(result);
            }
            (engine.clone(), output)
        })
        .collect()
}
fn query_contains(query: &str, id: &str) -> bool {
    // Python re.IGNORECASE is single-character matching, not full casefold:
    // U+0130 cannot expand into i + combining dot to match a two-char pattern.
    let query: String = query.chars().map(crate::text::ignorecase).collect();
    let id: String = id.chars().map(crate::text::ignorecase).collect();
    query.match_indices(&id).any(|(i, s)| {
        let word = |c: char| crate::text::word(c) || c == '.';
        !query[..i].chars().next_back().is_some_and(word)
            && !query[i + s.len()..].chars().next().is_some_and(word)
    })
}

fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
        Value::Number(n) => n.as_f64().is_none_or(|n| n != 0.0),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn requested_revision_uses_regex_ignorecase_without_expansion() {
        assert!(!super::query_contains(
            "cs.İI/0309136v1",
            "cs.i\u{307}i/0309136v1"
        ));
        assert!(super::query_contains("CS.KI/0309136V1", "cs.Kı/0309136v1"));
    }
}
