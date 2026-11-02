//! Functional native HTML wire surface. T03 owns the complete portal visual and
//! browser journey parity; this renderer keeps navigation and escaping usable.
use crate::native::{HtmlRenderer, PortalControls};
use ssx_domain::{SearchRequest, SearchResponse};
use ssx_json::Value;
/// Startup presentation snapshot; no request handler reads the process environment.
#[derive(Clone, Copy, Default)]
pub enum PortalTheme {
    #[default]
    Dark,
    Darker,
}
impl PortalTheme {
    fn parse(value: &str) -> Self {
        if value
            .trim_matches(ssx_json::python_whitespace)
            .to_lowercase()
            == "darker"
        {
            Self::Darker
        } else {
            Self::Dark
        }
    }
    fn text(self) -> &'static str {
        match self {
            Self::Dark => "dark",
            Self::Darker => "darker",
        }
    }
}
#[derive(Clone, Copy, Default)]
pub struct PortalOptions {
    pub theme: PortalTheme,
}
impl PortalOptions {
    pub fn from_environment(environment: &std::collections::BTreeMap<String, String>) -> Self {
        Self {
            theme: PortalTheme::parse(
                environment
                    .get("SLOPSEARX_PORTAL_DEFAULT_THEME")
                    .map_or("dark", String::as_str),
            ),
        }
    }
}
#[derive(Default)]
pub struct PublicHtml {
    theme: String,
}
impl PublicHtml {
    pub fn new(theme: &str) -> Self {
        Self::from_options(PortalOptions {
            theme: PortalTheme::parse(theme),
        })
    }
    pub fn from_options(options: PortalOptions) -> Self {
        Self {
            theme: options.theme.text().into(),
        }
    }
    fn document(&self, title: &str, body: &str) -> String {
        let theme = if self.theme.is_empty() {
            "dark"
        } else {
            &self.theme
        };
        let darker = theme == "darker";
        let (label, accessible_name, pressed) = if darker {
            ("Dark mode", "Use dark mode", "true")
        } else {
            ("Darker mode", "Use darker mode", "false")
        };
        format!(
            "<!doctype html><html lang=\"en\" data-default-theme=\"{theme}\" data-theme=\"{theme}\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>{}</title><style>:root{{--page-bg:#191a18;--page-fg:#f4f0e8;--muted:#bbb;color-scheme:dark}}:root[data-theme=darker]{{--page-bg:#0b0c0b}}html,body{{background-color:var(--page-bg);color:var(--page-fg)}}body{{max-width:70rem;margin:3rem auto;padding:0 1rem;font:1rem/1.6 system-ui}}a{{color:#e5b567}}form{{display:grid;grid-template-columns:repeat(auto-fit,minmax(min(100%,13rem),1fr));gap:1rem;align-items:end}}form label{{display:block}}form input,form select,form button{{box-sizing:border-box;width:100%;font:inherit;padding:.7rem}}article{{margin:2rem 0;border-top:1px solid #555}}small{{color:var(--muted)}}</style></head><body><header><a href=\"/\">SlopSearX</a> <button type=\"button\" data-theme-toggle aria-label=\"{accessible_name}\" aria-pressed=\"{pressed}\">{label}</button></header><main>{body}</main><script>(()=>{{const root=document.documentElement;const button=document.querySelector('[data-theme-toggle]');const configured=root.dataset.defaultTheme;const valid=theme=>theme==='dark'||theme==='darker';const apply=theme=>{{root.dataset.theme=theme;const darker=theme==='darker';button.textContent=darker?'Dark mode':'Darker mode';button.setAttribute('aria-label',darker?'Use dark mode':'Use darker mode');button.setAttribute('aria-pressed',String(darker))}};let initial=valid(configured)?configured:'dark';try{{const stored=window.localStorage.getItem('slopsearx-theme');if(valid(stored))initial=stored}}catch(_){{}}apply(initial);button.addEventListener('click',()=>{{const next=root.dataset.theme==='darker'?'dark':'darker';apply(next);try{{window.localStorage.setItem('slopsearx-theme',next)}}catch(_){{}}}});for(const form of document.querySelectorAll('[data-search-form]')){{const relative=form.querySelector('[name=time_range]');const absolute=form.querySelectorAll('[name=date_from],[name=date_to]');if(relative)relative.addEventListener('change',()=>{{if(relative.value)absolute.forEach(input=>input.value='')}});absolute.forEach(input=>input.addEventListener('input',()=>{{if(input.value&&relative)relative.value=''}}));form.addEventListener('formdata',event=>{{for(const select of form.querySelectorAll('select[multiple][data-comma-values]')){{const values=Array.from(select.selectedOptions,option=>option.value);event.formData.delete(select.name);if(values.length)event.formData.append(select.name,values.join(','))}}for(const input of form.querySelectorAll('input:not([type=hidden]):not([name=q]),select'))if(!input.value)event.formData.delete(input.name)}})}}}})();</script></body></html>",
            escape(title),
            accessible_name = accessible_name,
            pressed = pressed,
            label = label,
            body = body,
            theme = theme,
        )
    }
}
pub fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}
fn category_choices(discovery: &Value) -> Vec<(String, String)> {
    discovery
        .get("categories")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(|value| (value.to_owned(), value.to_owned()))
        .collect()
}
fn engine_choices(discovery: &Value) -> Vec<(String, String)> {
    discovery
        .get("engines")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|engine| {
            let name = engine.get("name")?.as_str()?;
            (engine.get("enabled").and_then(Value::as_bool) == Some(true)).then(|| {
                let label = engine
                    .get("display_name")
                    .and_then(Value::as_str)
                    .filter(|label| !label.is_empty())
                    .unwrap_or(name);
                (name.to_owned(), label.to_owned())
            })
        })
        .collect()
}
fn scope_select(
    name: &str,
    label: &str,
    choices: &[(String, String)],
    selected: &[String],
) -> String {
    if choices.is_empty() {
        return String::new();
    }
    let options = choices
        .iter()
        .map(|(value, label)| {
            let selected = if selected.contains(value) {
                " selected"
            } else {
                ""
            };
            format!(
                "<option value=\"{}\"{selected}>{}</option>",
                escape(value),
                escape(label)
            )
        })
        .collect::<String>();
    format!(
        "<label for=\"{name}\">{label}</label><select id=\"{name}\" name=\"{name}\" multiple size=\"4\" data-comma-values>{options}</select>"
    )
}
fn form(query: &str, request: Option<&SearchRequest>, controls: PortalControls<'_>) -> String {
    let categories = category_choices(controls.discovery);
    let engines = engine_choices(controls.discovery);
    let selected_categories = request
        .and_then(|r| r.categories.as_deref())
        .unwrap_or_default();
    let selected_engines = request
        .and_then(|r| r.engines.as_deref())
        .unwrap_or_default();
    let time_range = request
        .and_then(|r| r.time_range.as_deref())
        .unwrap_or_default();
    let date_from = request
        .and_then(|r| r.date_from.as_deref())
        .unwrap_or_default();
    let date_to = request
        .and_then(|r| r.date_to.as_deref())
        .unwrap_or_default();
    let relative_options = ["day", "week", "month", "year"]
        .into_iter()
        .map(|value| {
            let selected = if time_range == value { " selected" } else { "" };
            format!("<option value=\"{value}\"{selected}>{value}</option>")
        })
        .collect::<String>();
    let language = request.map_or("en", |r| r.language.as_str());
    let safesearch = request.map_or(0, |r| r.safesearch);
    format!(
        "<form class=\"search-form\" action=\"/search\" method=\"get\" data-search-form><label for=\"q\">Search</label><input id=\"q\" name=\"q\" value=\"{}\" required>{}<label for=\"time_range\">Relative time range</label><select id=\"time_range\" name=\"time_range\"><option value=\"\">Any time</option>{}</select><label for=\"date_from\">Publication date from</label><input id=\"date_from\" name=\"date_from\" type=\"date\" value=\"{}\"><label for=\"date_to\">Publication date to</label><input id=\"date_to\" name=\"date_to\" type=\"date\" value=\"{}\"><input type=\"hidden\" name=\"language\" value=\"{}\"><input type=\"hidden\" name=\"safesearch\" value=\"{}\"><input type=\"hidden\" name=\"format\" value=\"html\"><button type=\"submit\">Search</button></form>",
        escape(query),
        format!(
            "{}{}",
            scope_select("engines", "Sources", &engines, selected_engines),
            scope_select("categories", "Categories", &categories, selected_categories)
        ),
        relative_options,
        escape(date_from),
        escape(date_to),
        escape(language),
        safesearch
    )
}
fn date_status(meta: &Value) -> String {
    let Some(proof) = meta.get("date_filter") else {
        return String::new();
    };
    let field = |section: &str, name: &str| {
        let value = if section.is_empty() {
            proof.get(name)
        } else {
            proof.get(section).and_then(|value| value.get(name))
        };
        value.and_then(Value::as_str).unwrap_or("")
    };
    let bounds = match (field("applied", "date_from"), field("applied", "date_to")) {
        ("", "") => "no absolute bounds".to_owned(),
        (from, "") => format!("on or after {from}"),
        ("", to) => format!("on or before {to}"),
        (from, to) => format!("from {from} through {to}"),
    };
    let strings = |value: Option<&Value>| {
        value
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default()
    };
    let enforced_by = strings(proof.get("enforced_by"));
    let engines = strings(proof.get("engines"));
    format!(
        "<section aria-label=\"Publication date filter enforcement\" data-date-filter-status><h2>Publication date filter</h2><p>Requested publications {}. Status: {}. Enforced by: {}. Selected sources: {}.</p><p>{}</p></section>",
        escape(&bounds),
        escape(field("", "status")),
        escape(&enforced_by),
        escape(&engines),
        escape(field("", "reason"))
    )
}
fn machine_link(params: &[(&str, String)], format: &str) -> String {
    let mut serializer = url::form_urlencoded::Serializer::new(String::from("/search?"));
    for (key, value) in params {
        serializer.append_pair(key, value);
    }
    serializer.append_pair("format", format);
    escape(&serializer.finish())
}
impl HtmlRenderer for PublicHtml {
    fn landing(&self, controls: PortalControls<'_>) -> String {
        self.document(
            "SlopSearX",
            &format!(
                "<h1>Search <em>SlopSearX.</em></h1>{}",
                form("", None, controls)
            ),
        )
    }
    fn error(
        &self,
        error: &str,
        message: &str,
        field: Option<&str>,
        controls: PortalControls<'_>,
    ) -> String {
        self.document(
            "Search error",
            &format!(
                "<h1>Search error</h1><p role=\"alert\">{}: {}</p>{}{}",
                escape(error),
                escape(message),
                field
                    .map(|f| format!("<p>Field: {}</p>", escape(f)))
                    .unwrap_or_default(),
                form("", None, controls)
            ),
        )
    }
    fn search(
        &self,
        response: &SearchResponse,
        meta: &Value,
        request: &SearchRequest,
        controls: PortalControls<'_>,
    ) -> String {
        let mut body = format!(
            "<h1>Search results for {}</h1>{}<p>{} results</p>",
            escape(&response.query),
            form(&response.query, Some(request), controls),
            response.results.len()
        );
        if response.all_unresponsive {
            body.push_str("<p role=\"alert\">Search sources are temporarily unavailable.</p>");
        }
        body.push_str(&date_status(meta));
        for result in &response.results {
            body.push_str("<article><h2>");
            let parsed = ssx_application::parsed_url(&result.url);
            let safe = parsed
                .as_ref()
                .is_some_and(|p| matches!(p[0].as_str(), "http" | "https") && !p[1].is_empty());
            if safe {
                body.push_str(&format!(
                    "<a href=\"{}\" rel=\"noreferrer noopener\">{}</a>",
                    escape(&result.url),
                    escape(&result.title)
                ));
            } else {
                body.push_str(&escape(&result.title));
            }
            body.push_str(&format!(
                "</h2><p>{}</p><small>{}</small></article>",
                escape(&result.content),
                escape(&result.engine)
            ));
        }
        let mut params = vec![("q", response.query.clone())];
        if let Some(categories) = &request.categories {
            params.push(("categories", categories.join(",")));
        }
        if let Some(engines) = &request.engines {
            params.push(("engines", engines.join(",")));
        }
        for (k, v) in [
            ("time_range", &request.time_range),
            ("date_from", &request.date_from),
            ("date_to", &request.date_to),
        ] {
            if let Some(v) = v {
                params.push((k, v.clone()));
            }
        }
        params.push(("language", request.language.clone()));
        params.push(("safesearch", request.safesearch.to_string()));
        params.push(("pageno", request.page.to_string()));
        let link = |page: &ssx_domain::SearchPage| {
            let mut next = params.clone();
            if let Some((_, value)) = next.iter_mut().find(|(key, _)| *key == "pageno") {
                *value = page.to_string();
            }
            let mut serializer = url::form_urlencoded::Serializer::new(String::from("/search?"));
            for (k, v) in &next {
                serializer.append_pair(k, v);
            }
            escape(&serializer.finish())
        };
        body.push_str("<nav aria-label=\"Search pages\">");
        if let Some(previous) = request.page.previous_page() {
            body.push_str(&format!("<a href=\"{}\">← Previous</a> ", link(&previous)));
        }
        if let Some(next) = request.page.next_page() {
            if next.as_str().len() <= crate::native::MAX_PAGE_DIGITS {
                body.push_str(&format!("<a href=\"{}\">Try next page →</a>", link(&next)));
            } else {
                // Intentional bounded hardening: Python raises while formatting
                // this4301-digit neighbor. Retain exact current/previous values,
                // but never emit a link that our unchanged HTTP grammar rejects.
                body.push_str(&format!("<span aria-current=\"page\">Page {}</span> <span role=\"link\" aria-disabled=\"true\">Next page unavailable (page number limit)</span>", request.page));
            }
        }
        body.push_str("</nav>");
        body.push_str("<nav aria-label=\"Machine-readable formats\">");
        for format in ["json", "yaml"] {
            if format == "yaml" || controls.formats.contains(format) {
                body.push_str(&format!(
                    "<a href=\"{}\">{}</a> ",
                    machine_link(&params, format),
                    format.to_uppercase()
                ));
            }
        }
        if request.date_from.is_none() && request.date_to.is_none() {
            for format in ["csv", "rss"] {
                if controls.formats.contains(format) {
                    body.push_str(&format!(
                        "<a href=\"{}\">{}</a> ",
                        machine_link(&params, format),
                        format.to_uppercase()
                    ));
                }
            }
        } else {
            body.push_str("<span>CSV and RSS are unavailable for bounded publication-date searches because they cannot carry the enforcement proof.</span>");
        }
        body.push_str("</nav>");
        self.document(&format!("Search results for {}", response.query), &body)
    }
}

#[cfg(test)]
mod date_portal_tests {
    use super::*;
    use ssx_domain::PublicationDateProof;

    fn controls<'a>(
        discovery: &'a Value,
        formats: &'a std::collections::BTreeSet<String>,
    ) -> PortalControls<'a> {
        PortalControls { discovery, formats }
    }
    fn full_discovery() -> Value {
        ssx_json::json!({
            "categories": ["science", "general"],
            "engines": [{"name":"openalex", "display_name":"OpenAlex", "enabled":true}]
        })
    }

    #[test]
    fn bounded_portal_view_preserves_scope_and_displays_machine_proof() {
        let request = SearchRequest {
            query: "climate & change".into(),
            engines: Some(vec!["openalex".into()]),
            page: ssx_domain::SearchPage::new("2").unwrap(),
            date_from: Some("2024-02-29".into()),
            date_to: Some("2024-02-29".into()),
            ..Default::default()
        };
        let response = SearchResponse {
            query: request.query.clone(),
            date_filter: Some(
                PublicationDateProof::openalex(request.date_from.clone(), request.date_to.clone())
                    .unwrap(),
            ),
            ..Default::default()
        };
        let meta = crate::formats::meta(&response, &[]);
        let discovery = full_discovery();
        let formats = ["html", "json", "yaml", "csv", "rss"]
            .into_iter()
            .map(str::to_owned)
            .collect();
        let html = PublicHtml::new("dark").search(
            &response,
            &meta,
            &request,
            controls(&discovery, &formats),
        );

        for expected in [
            "<form class=\"search-form\"",
            "value=\"climate &amp; change\"",
            "<select id=\"engines\" name=\"engines\" multiple",
            "<option value=\"openalex\" selected>OpenAlex</option>",
            "name=\"date_from\" type=\"date\" value=\"2024-02-29\"",
            "name=\"date_to\" type=\"date\" value=\"2024-02-29\"",
            "aria-label=\"Publication date filter enforcement\"",
            "Status: enforced",
            "Enforced by: upstream:openalex",
            "Selected sources: openalex",
            "2024-02-29 through 2024-02-29",
            "Publication dates enforced by the audited OpenAlex adapter.",
            "date_from=2024-02-29",
            "date_to=2024-02-29",
            "engines=openalex",
            "format=json",
            "format=yaml",
            "CSV and RSS are unavailable for bounded publication-date searches",
        ] {
            assert!(
                html.contains(expected),
                "missing portal output {expected:?}"
            );
        }
        for page in ["pageno=1", "pageno=3"] {
            let preserved_bounds = format!(
                "date_from=2024-02-29&amp;date_to=2024-02-29&amp;language=en&amp;safesearch=0&amp;{page}"
            );
            assert!(
                html.contains(&preserved_bounds),
                "pagination link did not preserve bounds: {page}"
            );
        }
        assert!(!html.contains("format=csv"));
        assert!(!html.contains("format=rss"));
    }

    #[test]
    fn unbounded_portal_view_retains_compatible_machine_formats() {
        let request = SearchRequest::default();
        let response = SearchResponse::default();
        let meta = crate::formats::meta(&response, &[]);
        let discovery = full_discovery();
        let formats = ["html", "json", "yaml", "csv", "rss"]
            .into_iter()
            .map(str::to_owned)
            .collect();
        let html = PublicHtml::new("dark").search(
            &response,
            &meta,
            &request,
            controls(&discovery, &formats),
        );
        for format in ["json", "yaml", "csv", "rss"] {
            assert!(html.contains(&format!("format={format}")));
        }
        assert!(!html.contains("data-date-filter-status"));
    }

    #[test]
    fn relative_time_control_uses_supported_ranges_and_preserves_selection() {
        let request = SearchRequest {
            time_range: Some("week".into()),
            ..Default::default()
        };
        let discovery = full_discovery();
        let formats = std::collections::BTreeSet::new();
        let html = form("climate", Some(&request), controls(&discovery, &formats));
        for range in ["day", "week", "month", "year"] {
            assert!(html.contains(&format!("<option value=\"{range}\"")));
        }
        assert!(html.contains("<option value=\"week\" selected>week</option>"));
    }

    #[test]
    fn portal_controls_only_offer_exposed_scopes_and_enabled_formats() {
        let discovery = ssx_json::json!({
            "categories": ["science"],
            "engines": [
                {"name":"openalex", "display_name":"OpenAlex", "enabled":true},
                {"name":"disabled-source", "display_name":"Hidden Source", "enabled":false}
            ]
        });
        let formats = ["html", "json", "rss"]
            .into_iter()
            .map(str::to_owned)
            .collect();
        let request = SearchRequest {
            query: "climate".into(),
            categories: Some(vec!["science".into(), "unexposed-category".into()]),
            engines: Some(vec!["openalex".into(), "unexposed-source".into()]),
            ..Default::default()
        };
        let response = SearchResponse::default();
        let meta = crate::formats::meta(&response, &[]);
        let html = PublicHtml::new("dark").search(
            &response,
            &meta,
            &request,
            controls(&discovery, &formats),
        );

        assert!(html.contains("<option value=\"science\" selected>science</option>"));
        assert!(html.contains("<option value=\"openalex\" selected>OpenAlex</option>"));
        assert!(!html.contains("unexposed-category"));
        assert!(!html.contains("unexposed-source"));
        assert!(!html.contains("Hidden Source"));
        assert!(html.contains("format=json"));
        assert!(html.contains("format=rss"));
        assert!(html.contains("format=yaml"));
        assert!(!html.contains("format=csv"));
    }

    #[test]
    fn publication_date_proof_text_is_html_escaped() {
        let proof =
            PublicationDateProof::openalex(Some("<script>alert(1)</script>".into()), None).unwrap();
        let meta = ssx_json::json!({"date_filter": proof});
        let html = date_status(&meta);
        assert!(html.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
        assert!(!html.contains("<script>"));
    }
}
