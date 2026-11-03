//! Public, server-rendered search portal over the shared native HTTP boundary.
//! Rendering is pure: it cannot route, dispatch, retrieve, or acquire ownership.
use crate::{
    HttpState,
    html::escape,
    native::{HtmlRenderer, HtmlRequestState},
};
use ssx_application::entity_projection::Collector;
use ssx_domain::{SearchPage, SearchRequest, SearchResponse, SearchResult};
use ssx_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write,
    sync::Arc,
};

pub const STYLE: &str = include_str!("portal/style.css");
pub const SCRIPT: &str = include_str!("portal/behavior.js");

/// A presentation snapshot built only from already-redacted HTTP discovery.
pub struct PublicPortal {
    theme: &'static str,
    categories: Vec<String>,
    labels: BTreeMap<String, String>,
    declarations: BTreeMap<String, BTreeMap<String, String>>,
    public_sources: usize,
    json_enabled: bool,
}
impl Default for PublicPortal {
    fn default() -> Self {
        Self::new(
            &Value::Null,
            &BTreeSet::from(["html".into(), "json".into()]),
            "dark",
        )
    }
}
impl HttpState {
    /// Default public-portal composition for the native service. An injected
    /// renderer remains available to historical qualification and other callers.
    pub fn with_public_portal(mut self, theme: &str) -> Self {
        self.html = Arc::new(PublicPortal::new(&self.discovery, &self.formats, theme));
        self
    }
}
impl PublicPortal {
    pub fn new(discovery: &Value, formats: &BTreeSet<String>, theme: &str) -> Self {
        let mut categories = BTreeSet::new();
        let mut labels = BTreeMap::new();
        let mut declarations = BTreeMap::new();
        let mut public_sources = 0;
        for engine in discovery["engines"].as_array().into_iter().flatten() {
            let Some(name) = engine["name"].as_str() else {
                continue;
            };
            labels.insert(
                name.into(),
                engine["display_name"]
                    .as_str()
                    .filter(|s| !s.is_empty())
                    .map_or_else(|| label(name), str::to_owned),
            );
            declarations.insert(
                name.into(),
                engine["enforced_filters"]
                    .as_object()
                    .into_iter()
                    .flatten()
                    .filter_map(|(k, v)| v.as_str().map(|v| (k.clone(), v.to_owned())))
                    .collect(),
            );
            if engine["sensitive"].as_bool() == Some(true)
                || engine["enabled"].as_bool() == Some(false)
            {
                continue;
            }
            public_sources += 1;
            categories.extend(
                engine["categories"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .filter(|s| !s.trim().is_empty())
                    .map(str::to_owned),
            );
        }
        Self {
            theme: if theme
                .trim_matches(ssx_json::python_whitespace)
                .eq_ignore_ascii_case("darker")
            {
                "darker"
            } else {
                "dark"
            },
            categories: categories.into_iter().collect(),
            labels,
            declarations,
            public_sources,
            json_enabled: formats.contains("json"),
        }
    }
    fn source_label(&self, name: &str) -> String {
        self.labels
            .get(name)
            .cloned()
            .unwrap_or_else(|| label(name))
    }
    fn document(&self, title: &str, body: &str, invalid_field: Option<&str>) -> String {
        let darker = self.theme == "darker";
        format!(
            "<!doctype html><html lang=\"en\" data-default-theme=\"{}\" data-theme=\"{}\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><meta name=\"color-scheme\" content=\"dark\"><title>{}</title><style>{STYLE}</style></head><body{}><a class=\"skip-link\" href=\"#portal-main\">Skip to search content</a><main class=\"portal\" id=\"portal-main\"><header class=\"masthead\"><a class=\"brand\" href=\"/\"><span class=\"brand-mark\" aria-hidden=\"true\"><span>⌁</span></span><span>SLOPSEARX</span></a><button class=\"theme-toggle\" type=\"button\" data-theme-toggle aria-label=\"{}\" aria-pressed=\"{}\">{}</button></header>{body}<footer class=\"portal-footer\"><span>Sources are configured by the operator.</span><span>Press / to focus search.</span></footer></main><script>{SCRIPT}</script></body></html>",
            self.theme,
            self.theme,
            escape(title),
            invalid_field
                .map(|f| format!(" data-invalid-field=\"{}\"", escape(f)))
                .unwrap_or_default(),
            if darker {
                "Use dark mode"
            } else {
                "Use darker mode"
            },
            darker,
            if darker { "Dark mode" } else { "Darker mode" }
        )
    }
    fn search_form(&self, state: &FormState, landing: bool, invalid: Option<&str>) -> String {
        let opening = if landing {
            "<div class=\"search-form\">"
        } else {
            "<form class=\"search-form\" action=\"/search\" method=\"get\">"
        };
        let closing = if landing { "</div>" } else { "</form>" };
        format!(
            "<section class=\"search-panel\" aria-label=\"{}\">{opening}<label class=\"sr-only\" for=\"portal-query\">Search SlopSearX</label><input class=\"search-input\" id=\"portal-query\" name=\"q\" type=\"search\" value=\"{}\" placeholder=\"Search\" autocomplete=\"off\"{}{} required>{}<button class=\"search-button\" type=\"submit\">Search ↗</button>{closing}<span class=\"sr-only\" data-submit-status role=\"status\"></span>{}</section>",
            if landing { "Search" } else { "Refine search" },
            escape(state.get("q")),
            if landing { " autofocus" } else { "" },
            if matches!(invalid, Some("q" | "query")) {
                " aria-invalid=\"true\" aria-describedby=\"portal-error\""
            } else {
                ""
            },
            state.hidden(&["q", "pageno"], true),
            if landing {
                "<div class=\"search-hint\"><span>Enter a query</span><span><kbd>/</kbd> focus search</span></div>"
            } else {
                ""
            }
        )
    }
    fn scope_panel(
        &self,
        state: &FormState,
        response: Option<&SearchResponse>,
        meta: &Value,
        invalid_field: Option<&str>,
        embedded: bool,
    ) -> String {
        let opening = if embedded {
            "<div>"
        } else {
            "<form action=\"/search\" method=\"get\">"
        };
        let closing = if embedded { "</div>" } else { "</form>" };
        let selected = state.get("categories");
        let mut category_options = option("", "All sources", selected);
        if !selected.is_empty() && !self.categories.iter().any(|c| c == selected) {
            category_options.push_str(&option(
                selected,
                &format!("Selected: {}", label(selected)),
                selected,
            ));
        }
        for category in &self.categories {
            category_options.push_str(&option(category, &label(category), selected));
        }
        let time_options = [
            ("", "Any time"),
            ("day", "Past day"),
            ("week", "Past week"),
            ("month", "Past month"),
            ("year", "Past year"),
        ];
        let mut time = time_options
            .into_iter()
            .map(|(v, l)| option(v, l, state.get("time_range")))
            .collect::<String>();
        if !time_options
            .iter()
            .any(|(value, _)| *value == state.get("time_range"))
        {
            time.push_str(&option(
                state.get("time_range"),
                &format!("Submitted: {}", state.get("time_range")),
                state.get("time_range"),
            ));
        }
        let selected_engines = response
            .map(|r| r.scope.selected_engines.as_slice())
            .unwrap_or(&[]);
        let strict_available = !selected_engines.is_empty()
            && selected_engines.iter().all(|e| {
                self.declarations
                    .get(e)
                    .and_then(|d| d.get("safesearch"))
                    .is_some_and(|v| matches!(v.as_str(), "upstream" | "local"))
            });
        let mut safe = option("0", "Off", state.get("safesearch"));
        safe.push_str(&option("1", "Moderate", state.get("safesearch")));
        let _ = write!(
            safe,
            "<option value=\"2\"{}{}>Strict{}</option>",
            if state.get("safesearch") == "2" {
                " selected"
            } else {
                ""
            },
            if strict_available { "" } else { " disabled" },
            if strict_available {
                ""
            } else {
                " (unavailable for this scope)"
            }
        );
        if !["0", "1", "2"].contains(&state.get("safesearch")) {
            safe.push_str(&option(
                state.get("safesearch"),
                &format!("Submitted: {}", state.get("safesearch")),
                state.get("safesearch"),
            ));
        }
        let mut statuses = String::new();
        for (key, name) in [
            ("language", "language"),
            ("time_range", "time"),
            ("safesearch", "SafeSearch"),
            ("date_from", "published from"),
            ("date_to", "published through"),
        ] {
            let entry = &meta["enforcement"][key];
            if entry.is_object() {
                let status = entry["status"].as_str().unwrap_or("unsupported");
                let text = match status {
                    "enforced" => "Applied",
                    "partially_enforced" => "Passed to some sources",
                    "rejected" => "Rejected",
                    _ => "Not supported by this scope",
                };
                let requested = if matches!(key, "date_from" | "date_to") {
                    format!(" ({})", entry["requested"].as_str().unwrap_or(""))
                } else {
                    String::new()
                };
                let _ = write!(
                    statuses,
                    "<p class=\"enforcement\" data-filter=\"{key}\" data-status=\"{}\"><strong>{name}{}: {}</strong> — {text}. {}</p>",
                    escape(status),
                    escape(&requested),
                    escape(&status.replace('_', " ")),
                    escape(
                        entry["reason"]
                            .as_str()
                            .unwrap_or("No enforcement declaration is available.")
                    )
                );
            }
        }
        let invalid = |name: &str| {
            if invalid_field == Some(name) {
                " aria-invalid=\"true\" aria-describedby=\"portal-error\""
            } else {
                ""
            }
        };
        let mut note = String::new();
        if !state.get("engines").is_empty() {
            let _ = write!(
                note,
                "<p class=\"scope-note\">Explicit sources: {}. Explicit sources take precedence over categories.</p>",
                escape(state.get("engines"))
            );
        }
        if !strict_available {
            note.push_str("<p class=\"scope-note\" id=\"strict-safesearch-note\">Strict SafeSearch is unavailable because this scope cannot enforce it.</p>");
        } else {
            note.push_str("<p class=\"scope-note\" id=\"strict-safesearch-note\">Strict SafeSearch is available for this scope.</p>");
        }
        let correction=invalid_field.filter(|field|["engines","language","pageno","interactive_timeout_ms"].contains(field)).map(|field|format!("<label class=\"scope-field\">{}<input name=\"{}\" value=\"{}\" aria-invalid=\"true\" aria-describedby=\"portal-error\"></label>",escape(&label(field)),escape(field),escape(state.get(field)))).unwrap_or_default();
        if let Some(response) = response {
            for warning in &response.scope.warnings {
                let _ = write!(note, "<p class=\"scope-note\">{}</p>", escape(warning));
            }
        }
        format!(
            "<details class=\"scope-panel\" open><summary><span>Scope and filters</span><span class=\"scope-summary\">{}</span></summary>{opening}{}<div class=\"scope-grid\"><label class=\"scope-field\">Scope<select name=\"categories\"{}>{category_options}</select></label><label class=\"scope-field\">Time range<select name=\"time_range\"{}>{time}</select></label><label class=\"scope-field\">SafeSearch<select name=\"safesearch\" aria-describedby=\"strict-safesearch-note\"{}>{safe}</select></label><label class=\"scope-field\">Published from<input name=\"date_from\" type=\"{}\" value=\"{}\"{}></label><label class=\"scope-field\">Published through<input name=\"date_to\" type=\"{}\" value=\"{}\"{}></label>{correction}<button class=\"scope-apply\" type=\"submit\">Apply filters</button></div>{note}<div class=\"scope-status\" aria-live=\"polite\">{statuses}</div>{closing}</details>",
            escape(if selected.is_empty() {
                "All sources"
            } else {
                selected
            }),
            state.hidden(
                &[
                    "categories",
                    "time_range",
                    "safesearch",
                    "date_from",
                    "date_to",
                    "pageno"
                ],
                true
            ),
            invalid("categories"),
            invalid("time_range"),
            invalid("safesearch"),
            if invalid_field == Some("date_from") {
                "text"
            } else {
                "date"
            },
            escape(state.get("date_from")),
            invalid("date_from"),
            if invalid_field == Some("date_to") {
                "text"
            } else {
                "date"
            },
            escape(state.get("date_to")),
            invalid("date_to")
        )
    }
    fn result(
        &self,
        result: &SearchResult,
        index: usize,
        groups: &[Value],
        response: &SearchResponse,
    ) -> String {
        let title = if result.title.is_empty() {
            if result.url.is_empty() {
                "Untitled result"
            } else {
                &result.url
            }
        } else {
            &result.title
        };
        let safe = safe_url(&result.url);
        let parsed = safe.and_then(|u| url::Url::parse(u).ok());
        let domain = parsed
            .as_ref()
            .and_then(url::Url::host_str)
            .unwrap_or("unknown source");
        let path = parsed.as_ref().map_or("", |u| u.path());
        let engines = sources(result);
        let pills = engines
            .iter()
            .map(|e| {
                format!(
                    "<span class=\"result-pill\">{}</span>",
                    escape(&self.source_label(e))
                )
            })
            .collect::<String>();
        let mut body = format!(
            "<article class=\"result\" id=\"result-{}\" data-result-card><div class=\"result-kicker\"><span class=\"result-source-line\">{}</span><span class=\"result-path\">{}</span><span class=\"result-pill type\">{}</span>",
            index + 1,
            escape(domain),
            escape(path),
            escape(result_type(result))
        );
        if engines.len() > 1 {
            let _ = write!(
                body,
                "<span class=\"result-pill consensus\" title=\"Same result or identified work found by {} configured engines\">Matched {} sources</span>",
                engines.len(),
                engines.len()
            );
        }
        if let Some(url) = safe {
            let _ = write!(
                body,
                "</div><h2><a class=\"result-link\" href=\"{}\" target=\"_blank\" rel=\"noopener noreferrer\">{}</a></h2>",
                escape(url),
                escape(title)
            );
        } else {
            let _ = write!(
                body,
                "</div><h2><span class=\"result-title\">{}</span></h2><p class=\"scope-note\">This result has no safe web destination.</p>",
                escape(title)
            );
        }
        let _ = write!(
            body,
            "<p class=\"result-content\">{}</p>",
            content_markup(&result.content)
        );
        if let Some(payload) = result.payload.as_ref().filter(|p| p.is_object()) {
            let _ = write!(
                body,
                "<div class=\"result-special\"><span>{} / {}</span></div>",
                escape(payload["domain"].as_str().unwrap_or("specialist")),
                escape(payload["type"].as_str().unwrap_or("result"))
            );
        }
        if let Some(media) = &result.media
            && let Some(thumbnail) = media.thumbnail.as_deref().and_then(safe_url)
        {
            let _ = write!(
                body,
                "<figure class=\"result-media\"><img src=\"{}\" alt=\"{}\" loading=\"lazy\"><figcaption>{}</figcaption></figure>",
                escape(thumbnail),
                escape(title),
                escape(media.source.as_deref().unwrap_or(domain))
            );
        }
        let _ = write!(
            body,
            "<div class=\"result-meta\">{pills}<span>{}</span>",
            escape(&result.category)
        );
        if let Some(date) = result.published_date.as_deref().filter(|d| !d.is_empty()) {
            let _ = write!(
                body,
                "<time datetime=\"{}\">{}</time>",
                escape(date),
                escape(date.split('T').next().unwrap_or(date))
            );
        }
        body.push_str("</div><details class=\"result-explanation\"><summary>Why this result appeared</summary><ul class=\"explanation-list\">");
        let _ = write!(
            body,
            "<li><strong>Contributors:</strong><span class=\"explanation-sources\">{pills}</span> ({} configured sources)</li><li><strong>Ranking:</strong> {} Configured tier {}. The score controls ordering; it is not confidence.</li>",
            engines.len(),
            escape(ranking_text(&response.ranking_explanation)),
            result.tier
        );
        let selected = groups
            .iter()
            .filter(|g| {
                g["result_ids"].as_array().is_some_and(|ids| {
                    ids.iter().any(|id| {
                        id.as_str()
                            .and_then(|s| s.rsplit_once(':'))
                            .and_then(|(_, s)| s.parse::<usize>().ok())
                            == Some(index)
                    })
                })
            })
            .collect::<Vec<_>>();
        if selected.is_empty() {
            body.push_str("<li><strong>Entity grouping:</strong> <span class=\"explanation-status\" data-status=\"unavailable\">unavailable for this response</span>.</li>");
        }
        for group in selected {
            if group["entity_id"].is_null() {
                let _ = write!(
                    body,
                    "<li><strong>Entity grouping:</strong> unsupported for this result ({}).</li>",
                    escape(
                        group["reason"]
                            .as_str()
                            .unwrap_or("unsupported result metadata")
                    )
                    .replace('_', " ")
                );
                continue;
            }
            let identifiers = group["identifier"]
                .as_object()
                .into_iter()
                .flatten()
                .map(|(k, v)| {
                    format!(
                        "{}: {}",
                        k.replace('_', " "),
                        v.as_str().map_or_else(|| v.to_string(), str::to_owned)
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
            let members = group["result_ids"].as_array().map_or(0, Vec::len);
            let _ = write!(
                body,
                "<li><strong>Entity:</strong> <span class=\"explanation-status\" data-status=\"available\">{} — {}</span> · {members} results in this canonical response</li>",
                escape(group["namespace"].as_str().unwrap_or("unknown")),
                escape(&identifiers)
            );
            let conflicts = group["conflicting_fields"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(escape)
                .collect::<Vec<_>>()
                .join(", ");
            if !conflicts.is_empty() {
                let _ = write!(
                    body,
                    "<li><strong>Source-reported conflicts:</strong> {conflicts}. Inspect contributing results: "
                );
                for member in group["result_ids"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .filter_map(|s| s.rsplit_once(':'))
                    .filter_map(|(_, s)| s.parse::<usize>().ok())
                    .filter(|i| *i < response.results.len())
                {
                    let _ = write!(
                        body,
                        "<a class=\"explanation-link\" href=\"#result-{}\">#{}</a> ",
                        member + 1,
                        member + 1
                    );
                }
                body.push_str("</li>");
            }
        }
        let retrieval = ssx_application::retrieval::classify_retrieval_url(&result.url);
        if retrieval.status == "ok" {
            body.push_str("<li><strong>Retrieval handoff:</strong> <span class=\"explanation-status\" data-status=\"eligible\">structurally eligible</span> for downstream retrieval. The page has not been fetched or verified; a retriever must check the resolved destination.</li>");
        } else {
            let _ = write!(
                body,
                "<li><strong>Retrieval handoff:</strong> <span class=\"explanation-status\" data-status=\"ineligible\">not eligible</span> for the technical handoff ({}).</li>",
                escape(retrieval.reason.as_deref().unwrap_or(retrieval.status))
            );
        }
        body.push_str("</ul></details>");
        if let Some(url) = safe {
            let _ = write!(
                body,
                "<div class=\"result-actions\"><a class=\"result-action\" href=\"{}\" target=\"_blank\" rel=\"noopener noreferrer\">Open result ↗</a></div>",
                escape(url)
            );
        }
        body.push_str("</article>");
        body
    }
    fn render_error(
        &self,
        error: &str,
        message: &str,
        field: Option<&str>,
        state: &FormState,
    ) -> String {
        let mut body = format!(
            "<section class=\"results-shell\" aria-labelledby=\"error-title\"><div class=\"eyebrow\">Search request</div><h1 id=\"error-title\">{}</h1><div class=\"notice\" data-kind=\"error\" role=\"alert\" id=\"portal-error\"><strong>{}</strong>{}</div>",
            escape(error),
            escape(message),
            field
                .map(|f| format!("<p>Field: {}</p>", escape(f)))
                .unwrap_or_default()
        );
        body.push_str(&self.search_form(state, false, field));
        body.push_str(&self.scope_panel(state, None, &Value::Null, field, false));
        body.push_str("<p><a class=\"page-link\" href=\"/\">Return to search</a></p></section>");
        self.document("Search request needs attention", &body, field)
    }
}
impl HtmlRenderer for PublicPortal {
    fn landing(&self) -> String {
        let state = FormState::default();
        let mut body = format!(
            "<section class=\"hero\" aria-labelledby=\"hero-title\"><div><div class=\"eyebrow\">Search</div><h1 id=\"hero-title\">Search <em>SlopSearX.</em></h1><p class=\"hero-copy\">Search across the web and specialist sources configured for this instance.</p></div><aside class=\"hero-note\"><strong>{} configured public sources</strong>Results identify their source. If an engine cannot answer, the page says so.</aside></section>",
            self.public_sources
        );
        body.push_str("<form action=\"/search\" method=\"get\" id=\"portal-landing-search\">");
        body.push_str(&self.search_form(&state, true, None));
        body.push_str(&self.scope_panel(&state, None, &Value::Null, None, true));
        body.push_str("</form>");
        self.document("SlopSearX", &body, None)
    }
    fn error(&self, error: &str, message: &str, field: Option<&str>) -> String {
        self.render_error(error, message, field, &FormState::default())
    }
    fn error_with_request(
        &self,
        error: &str,
        message: &str,
        field: Option<&str>,
        request: &HtmlRequestState,
    ) -> String {
        self.render_error(
            error,
            message,
            field.or((error == "query_required").then_some("q")),
            &FormState::from_parameters(request.parameters()),
        )
    }
    fn search(&self, response: &SearchResponse, meta: &Value, request: &SearchRequest) -> String {
        let state = FormState::from_request(request);
        let responsive = response
            .engine_outcomes
            .iter()
            .filter(|o| o.status == "ok")
            .count();
        let mut body = format!(
            "<section class=\"results-shell\" aria-labelledby=\"results-title\"><div class=\"results-top\"><div><div class=\"eyebrow\">Search results</div><h1 id=\"results-title\">{}</h1></div><p class=\"summary\">{} results · {} ms<br>{} · {responsive} of {} sources answered</p></div>",
            escape(&response.query),
            response.results.len(),
            response.response_time_ms,
            escape(if state.get("categories").is_empty() {
                "All sources"
            } else {
                state.get("categories")
            }),
            response.scope.selected_engines.len()
        );
        body.push_str(&self.search_form(&state, false, None));
        body.push_str(&self.scope_panel(&state, Some(response), meta, None, false));
        let failures = response
            .engine_outcomes
            .iter()
            .filter(|o| o.status != "ok")
            .map(|o| format!("{} ({})", self.source_label(&o.engine), o.status))
            .collect::<Vec<_>>()
            .join(", ");
        let empty = response
            .empty_engines
            .iter()
            .filter_map(|e| e.first())
            .map(|e| self.source_label(e))
            .collect::<Vec<_>>()
            .join(", ");
        if response.all_unresponsive {
            let _ = write!(
                body,
                "<div class=\"notice\" data-kind=\"error\" role=\"alert\"><strong>No configured source returned a result.</strong> You can retry this search. {}</div>",
                escape(&failures)
            );
        } else if response.partial || !failures.is_empty() {
            let _ = write!(
                body,
                "<div class=\"notice\" role=\"status\"><strong>Some sources could not answer this search.</strong> Unavailable: {}. No results: {}. The results below are still usable.</div>",
                escape(&failures),
                escape(&empty)
            );
        } else if !empty.is_empty() {
            let _ = write!(
                body,
                "<div class=\"notice\" role=\"status\"><strong>No results from some sources.</strong> No results: {}. Other results remain usable.</div>",
                escape(&empty)
            );
        }
        if !response.scope.jev_added_engines.is_empty() {
            let _ = write!(
                body,
                "<p class=\"scope-note\">Specialists added by Jev: {}</p>",
                escape(
                    &response
                        .scope
                        .jev_added_engines
                        .iter()
                        .map(|e| self.source_label(e))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            );
        }
        if let Some(notes) = meta["advisories"].as_array().filter(|a| !a.is_empty()) {
            body.push_str("<details class=\"notice\"><summary>Source availability</summary><ul>");
            for note in notes.iter().take(3) {
                let _ = write!(
                    body,
                    "<li>{}</li>",
                    escape(note["message"].as_str().unwrap_or(""))
                );
            }
            body.push_str("</ul></details>");
        }
        body.push_str("<div class=\"results-layout\"><div class=\"results-main\"><div class=\"results-list\" aria-live=\"polite\">");
        let mut collector = Collector::new("portal-response", 2);
        for (i, result) in response.results.iter().enumerate() {
            collector.feed(i, result);
        }
        let (groups, _) = collector.finish();
        for (i, result) in response.results.iter().enumerate() {
            body.push_str(&self.result(result, i, &groups, response));
        }
        if response.results.is_empty() {
            let _ = write!(
                body,
                "<div class=\"empty\"><h2>No results found</h2><p>No results found for <strong>{}</strong>. Try editing the query or broadening its scope.</p>",
                escape(&response.query)
            );
            if !response.suggestions.is_empty() {
                body.push_str("<div class=\"suggestions\" aria-label=\"Suggestions\">");
                for suggestion in response
                    .suggestions
                    .iter()
                    .filter(|s| !s.trim().is_empty())
                    .take(5)
                {
                    let mut next = state.clone();
                    next.set("q", suggestion);
                    let _ = write!(
                        body,
                        "<a href=\"{}\">{}</a>",
                        escape(&next.url(None, Some(&SearchPage::default()))),
                        escape(suggestion)
                    );
                }
                body.push_str("</div>");
            }
            body.push_str("</div>");
        }
        body.push_str("</div>");
        if request.page.previous_page().is_some() || !response.results.is_empty() {
            body.push_str("<nav class=\"pagination\" aria-label=\"Search pages\">");
            if let Some(previous) = request.page.previous_page() {
                let _ = write!(
                    body,
                    "<a class=\"page-link\" aria-label=\"Previous result page\" href=\"{}\">← Previous</a>",
                    escape(&state.url(None, Some(&previous)))
                );
            } else {
                body.push_str("<span class=\"page-spacer\" aria-hidden=\"true\"></span>");
            }
            let _ = write!(
                body,
                "<span class=\"page-current\" aria-current=\"page\">Page {}{}</span>",
                request.page,
                if response.results.is_empty() {
                    ""
                } else {
                    " · more may be available"
                }
            );
            if !response.results.is_empty() {
                if let Some(next) = request
                    .page
                    .next_page()
                    .filter(|p| p.as_str().len() <= crate::native::MAX_PAGE_DIGITS)
                {
                    let _ = write!(
                        body,
                        "<a class=\"page-link\" aria-label=\"Try next result page\" href=\"{}\">Try next page →</a>",
                        escape(&state.url(None, Some(&next)))
                    );
                } else {
                    body.push_str("<span class=\"page-link\" role=\"link\" aria-disabled=\"true\">Next page unavailable (page number limit)</span>");
                }
            }
            body.push_str("</nav>");
        }
        body.push_str("</div><aside class=\"results-rail\" aria-label=\"Search summary\"><section class=\"rail-card\"><p class=\"rail-label\">Source status</p><ul class=\"rail-sources\">");
        let mut counts = BTreeMap::<String, usize>::new();
        for result in &response.results {
            for engine in sources(result) {
                *counts.entry(engine).or_default() += 1;
            }
        }
        for (engine, count) in counts {
            let _ = write!(
                body,
                "<li class=\"rail-source\"><span class=\"rail-source-name\">{}</span><span class=\"rail-source-count\">{count}</span></li>",
                escape(&self.source_label(&engine))
            );
        }
        if response.results.is_empty() {
            body.push_str("<li class=\"rail-source\">No responding sources returned results</li>");
        }
        body.push_str("</ul>");
        if self.json_enabled {
            let _ = write!(
                body,
                "<a class=\"rail-link\" href=\"{}\">Open JSON view ↗</a>",
                escape(&state.url(Some("json"), None))
            );
        }
        let _ = write!(
            body,
            "<a class=\"rail-link\" href=\"{}\">Open YAML view ↗</a></section></aside></div></section>",
            escape(&state.url(Some("yaml"), None))
        );
        self.document(
            &format!("Search results for {}", response.query),
            &body,
            None,
        )
    }
}

#[derive(Clone)]
struct FormState(BTreeMap<String, String>);
impl Default for FormState {
    fn default() -> Self {
        Self(BTreeMap::from([
            ("language".into(), "en".into()),
            ("safesearch".into(), "0".into()),
            ("pageno".into(), "1".into()),
        ]))
    }
}
impl FormState {
    fn from_parameters(parameters: &BTreeMap<String, String>) -> Self {
        let mut state = Self::default();
        state.0.extend(parameters.clone());
        state
    }
    fn get(&self, key: &str) -> &str {
        self.0.get(key).map_or("", String::as_str)
    }
    fn set(&mut self, key: &str, value: &str) {
        self.0.insert(key.into(), value.into());
    }
    fn from_request(request: &SearchRequest) -> Self {
        let mut s = Self::default();
        for (k, v) in [
            ("q", Some(request.query.clone())),
            (
                "categories",
                request.categories.as_ref().map(|v| v.join(",")),
            ),
            ("engines", request.engines.as_ref().map(|v| v.join(","))),
            ("language", Some(request.language.clone())),
            ("time_range", request.time_range.clone()),
            ("date_from", request.date_from.clone()),
            ("date_to", request.date_to.clone()),
            ("safesearch", Some(request.safesearch.to_string())),
            ("pageno", Some(request.page.to_string())),
            (
                "interactive_timeout_ms",
                request.interactive_timeout_ms.map(|n| n.to_string()),
            ),
        ] {
            if let Some(v) = v {
                s.set(k, &v);
            }
        }
        s
    }
    fn hidden(&self, exclude: &[&str], reset_page: bool) -> String {
        let mut out = String::new();
        for (k, v) in &self.0 {
            if !exclude.contains(&k.as_str()) && !v.is_empty() {
                let value = if reset_page && k == "pageno" { "1" } else { v };
                let _ = write!(
                    out,
                    "<input type=\"hidden\" name=\"{}\" value=\"{}\">",
                    escape(k),
                    escape(value)
                );
            }
        }
        if reset_page && exclude.contains(&"pageno") {
            out.push_str("<input type=\"hidden\" name=\"pageno\" value=\"1\">");
        }
        out
    }
    fn url(&self, format: Option<&str>, page: Option<&SearchPage>) -> String {
        let mut s = url::form_urlencoded::Serializer::new(String::from("/search?"));
        for (k, v) in &self.0 {
            if !v.is_empty() {
                s.append_pair(
                    k,
                    if k == "pageno" {
                        page.map_or(v.as_str(), SearchPage::as_str)
                    } else {
                        v
                    },
                );
            }
        }
        if let Some(format) = format {
            s.append_pair("format", format);
        }
        s.finish()
    }
}
fn option(value: &str, text: &str, selected: &str) -> String {
    format!(
        "<option value=\"{}\"{}>{}</option>",
        escape(value),
        if value == selected { " selected" } else { "" },
        escape(text)
    )
}
fn ranking_text(strategy: &str) -> &'static str {
    match strategy {
        "semantic_shortlist_rerank" => {
            "Semantic relevance orders the bounded shortlist across general and specialist sources. Displayed scores retain the original source-fusion arithmetic."
        }
        "tier_then_semantic_rerank" => {
            "Semantic relevance orders the bounded shortlist within configured source tiers. Displayed scores retain the original source-fusion arithmetic."
        }
        "tier_then_reciprocal_rank_fusion_k60" => {
            "Reciprocal rank fusion (k=60) combines each source's result order after configured source tiers."
        }
        _ => {
            "Cross-source presence counts configured sources returning the same URL or identified work after configured source tiers."
        }
    }
}
fn label(s: &str) -> String {
    s.replace(['_', '-', ':'], " ")
        .split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            chars.next().map_or_else(String::new, |c| {
                c.to_uppercase().collect::<String>() + chars.as_str()
            })
        })
        .collect::<Vec<_>>()
        .join(" ")
}
fn sources(result: &SearchResult) -> BTreeSet<String> {
    if result.engines.is_empty() {
        BTreeSet::from([result.engine.clone()])
            .into_iter()
            .filter(|s| !s.is_empty())
            .collect()
    } else {
        result
            .engines
            .iter()
            .filter(|s| !s.is_empty())
            .cloned()
            .collect()
    }
}
fn safe_url(raw: &str) -> Option<&str> {
    let parsed = url::Url::parse(raw).ok()?;
    (matches!(parsed.scheme(), "http" | "https") && parsed.host_str().is_some()).then_some(raw)
}
fn result_type(result: &SearchResult) -> &str {
    if let Some(media) = &result.media {
        return &media.media_type;
    }
    if result.payload.is_some() {
        return "Structured";
    }
    match result.category.split(':').next().unwrap_or("") {
        "science" => "Research",
        "github" => "Developer",
        "packages" => "Package",
        "security" => "Security",
        "jobs" => "Jobs",
        "news" => "News",
        "reference" => "Reference",
        "media" => "Media",
        _ => "General",
    }
}
/// Preserve only exact, attribute-free highlight pairs; all nested input is escaped.
pub fn content_markup(source: &str) -> String {
    let lower = source.to_ascii_lowercase();
    let mut output = String::new();
    let (mut cursor, mut search) = (0, 0);
    while search < source.len() {
        let candidate = ["strong", "em", "mark"]
            .into_iter()
            .filter_map(|tag| {
                lower[search..]
                    .find(&format!("<{tag}>"))
                    .map(|i| (search + i, tag))
            })
            .min_by_key(|(i, _)| *i);
        let Some((start, tag)) = candidate else { break };
        let begin = start + tag.len() + 2;
        let Some(end) = lower[begin..].find(&format!("</{tag}>")) else {
            search = begin;
            continue;
        };
        let end = begin + end;
        output.push_str(&escape(&source[cursor..start]));
        let _ = write!(output, "<{tag}>{}</{tag}>", escape(&source[begin..end]));
        cursor = end + tag.len() + 3;
        search = cursor;
    }
    output.push_str(&escape(&source[cursor..]));
    output
}
