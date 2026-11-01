//! Functional native HTML wire surface. T03 owns the complete portal visual and
//! browser journey parity; this renderer keeps navigation and escaping usable.
use crate::native::HtmlRenderer;
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
            "<!doctype html><html lang=\"en\" data-default-theme=\"{theme}\" data-theme=\"{theme}\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>{}</title><style>:root{{--page-bg:#191a18;--page-fg:#f4f0e8;--muted:#bbb;color-scheme:dark}}:root[data-theme=darker]{{--page-bg:#0b0c0b}}html,body{{background-color:var(--page-bg);color:var(--page-fg)}}body{{max-width:70rem;margin:3rem auto;padding:0 1rem;font:1rem/1.6 system-ui}}a{{color:#e5b567}}form{{display:grid;grid-template-columns:repeat(auto-fit,minmax(min(100%,13rem),1fr));gap:1rem;align-items:end}}form label{{display:block}}form input,form button{{box-sizing:border-box;width:100%;font:inherit;padding:.7rem}}article{{margin:2rem 0;border-top:1px solid #555}}small{{color:var(--muted)}}</style></head><body><header><a href=\"/\">SlopSearX</a> <button type=\"button\" data-theme-toggle aria-label=\"{accessible_name}\" aria-pressed=\"{pressed}\">{label}</button></header><main>{body}</main><script>(()=>{{const root=document.documentElement;const button=document.querySelector('[data-theme-toggle]');const configured=root.dataset.defaultTheme;const valid=theme=>theme==='dark'||theme==='darker';const apply=theme=>{{root.dataset.theme=theme;const darker=theme==='darker';button.textContent=darker?'Dark mode':'Darker mode';button.setAttribute('aria-label',darker?'Use dark mode':'Use darker mode');button.setAttribute('aria-pressed',String(darker))}};let initial=valid(configured)?configured:'dark';try{{const stored=window.localStorage.getItem('slopsearx-theme');if(valid(stored))initial=stored}}catch(_){{}}apply(initial);button.addEventListener('click',()=>{{const next=root.dataset.theme==='darker'?'dark':'darker';apply(next);try{{window.localStorage.setItem('slopsearx-theme',next)}}catch(_){{}}}});for(const form of document.querySelectorAll('[data-search-form]')){{form.addEventListener('submit',()=>{{for(const input of form.querySelectorAll('input:not([type=hidden]):not([name=q])'))if(!input.value)input.removeAttribute('name')}})}}}})();</script></body></html>",
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
fn form(query: &str, request: Option<&SearchRequest>) -> String {
    let categories = request
        .and_then(|r| r.categories.as_ref())
        .map(|v| v.join(","))
        .unwrap_or_default();
    let engines = request
        .and_then(|r| r.engines.as_ref())
        .map(|v| v.join(","))
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
    let language = request.map_or("en", |r| r.language.as_str());
    let safesearch = request.map_or(0, |r| r.safesearch);
    format!(
        "<form action=\"/search\" method=\"get\" data-search-form><label for=\"q\">Search</label><input id=\"q\" name=\"q\" value=\"{}\" required><label for=\"engines\">Sources (comma-separated)</label><input id=\"engines\" name=\"engines\" value=\"{}\" autocomplete=\"off\"><label for=\"categories\">Categories (comma-separated)</label><input id=\"categories\" name=\"categories\" value=\"{}\" autocomplete=\"off\"><label for=\"time_range\">Relative time range</label><input id=\"time_range\" name=\"time_range\" value=\"{}\"><label for=\"date_from\">Publication date from</label><input id=\"date_from\" name=\"date_from\" type=\"date\" value=\"{}\"><label for=\"date_to\">Publication date to</label><input id=\"date_to\" name=\"date_to\" type=\"date\" value=\"{}\"><input type=\"hidden\" name=\"language\" value=\"{}\"><input type=\"hidden\" name=\"safesearch\" value=\"{}\"><input type=\"hidden\" name=\"format\" value=\"html\"><button type=\"submit\">Search</button></form>",
        escape(query),
        escape(&engines),
        escape(&categories),
        escape(time_range),
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
    fn landing(&self) -> String {
        self.document(
            "SlopSearX",
            &format!("<h1>Search <em>SlopSearX.</em></h1>{}", form("", None)),
        )
    }
    fn error(&self, error: &str, message: &str, field: Option<&str>) -> String {
        self.document(
            "Search error",
            &format!(
                "<h1>Search error</h1><p role=\"alert\">{}: {}</p>{}{}",
                escape(error),
                escape(message),
                field
                    .map(|f| format!("<p>Field: {}</p>", escape(f)))
                    .unwrap_or_default(),
                form("", None)
            ),
        )
    }
    fn search(&self, response: &SearchResponse, meta: &Value, request: &SearchRequest) -> String {
        let mut body = format!(
            "<h1>Search results for {}</h1>{}<p>{} results</p>",
            escape(&response.query),
            form(&response.query, Some(request)),
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
            body.push_str(&format!(
                "<a href=\"{}\">{}</a> ",
                machine_link(&params, format),
                format.to_uppercase()
            ));
        }
        if request.date_from.is_none() && request.date_to.is_none() {
            for format in ["csv", "rss"] {
                body.push_str(&format!(
                    "<a href=\"{}\">{}</a> ",
                    machine_link(&params, format),
                    format.to_uppercase()
                ));
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

    #[test]
    fn bounded_portal_view_preserves_scope_and_displays_machine_proof() {
        let mut request = SearchRequest::default();
        request.query = "climate & change".into();
        request.engines = Some(vec!["openalex".into()]);
        request.date_from = Some("2024-02-29".into());
        request.date_to = Some("2024-02-29".into());
        let mut response = SearchResponse::default();
        response.query = request.query.clone();
        response.date_filter = Some(
            PublicationDateProof::openalex(request.date_from.clone(), request.date_to.clone())
                .unwrap(),
        );
        let meta = crate::formats::meta(&response, &[]);
        let html = PublicHtml::new("dark").search(&response, &meta, &request);

        for expected in [
            "value=\"climate &amp; change\"",
            "name=\"engines\" value=\"openalex\"",
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
        assert!(!html.contains("format=csv"));
        assert!(!html.contains("format=rss"));
    }

    #[test]
    fn unbounded_portal_view_retains_compatible_machine_formats() {
        let request = SearchRequest::default();
        let response = SearchResponse::default();
        let meta = crate::formats::meta(&response, &[]);
        let html = PublicHtml::new("dark").search(&response, &meta, &request);
        for format in ["json", "yaml", "csv", "rss"] {
            assert!(html.contains(&format!("format={format}")));
        }
        assert!(!html.contains("data-date-filter-status"));
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
