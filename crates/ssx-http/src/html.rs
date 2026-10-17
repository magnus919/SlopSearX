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
            "<!doctype html><html lang=\"en\" data-default-theme=\"{theme}\" data-theme=\"{theme}\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>{}</title><style>:root{{--page-bg:#191a18;--page-fg:#f4f0e8;--muted:#bbb;color-scheme:dark}}:root[data-theme=darker]{{--page-bg:#0b0c0b}}html,body{{background-color:var(--page-bg);color:var(--page-fg)}}body{{max-width:70rem;margin:3rem auto;padding:0 1rem;font:1rem/1.6 system-ui}}a{{color:#e5b567}}form{{display:flex;gap:1rem}}input{{flex:1}}input,button{{font:inherit;padding:.7rem}}article{{margin:2rem 0;border-top:1px solid #555}}small{{color:var(--muted)}}</style></head><body><header><a href=\"/\">SlopSearX</a> <button type=\"button\" data-theme-toggle aria-label=\"{accessible_name}\" aria-pressed=\"{pressed}\">{label}</button></header><main>{body}</main><script>(()=>{{const root=document.documentElement;const button=document.querySelector('[data-theme-toggle]');const configured=root.dataset.defaultTheme;const valid=theme=>theme==='dark'||theme==='darker';const apply=theme=>{{root.dataset.theme=theme;const darker=theme==='darker';button.textContent=darker?'Dark mode':'Darker mode';button.setAttribute('aria-label',darker?'Use dark mode':'Use darker mode');button.setAttribute('aria-pressed',String(darker))}};let initial=valid(configured)?configured:'dark';try{{const stored=window.localStorage.getItem('slopsearx-theme');if(valid(stored))initial=stored}}catch(_){{}}apply(initial);button.addEventListener('click',()=>{{const next=root.dataset.theme==='darker'?'dark':'darker';apply(next);try{{window.localStorage.setItem('slopsearx-theme',next)}}catch(_){{}}}})}})();</script></body></html>",
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
fn form(query: &str) -> String {
    format!(
        "<form action=\"/search\" method=\"get\"><label for=\"q\">Search</label><input id=\"q\" name=\"q\" value=\"{}\" required><button type=\"submit\">Search</button></form>",
        escape(query)
    )
}
impl HtmlRenderer for PublicHtml {
    fn landing(&self) -> String {
        self.document(
            "SlopSearX",
            &format!("<h1>Search <em>SlopSearX.</em></h1>{}", form("")),
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
                form("")
            ),
        )
    }
    fn search(&self, response: &SearchResponse, _meta: &Value, request: &SearchRequest) -> String {
        let mut body = format!(
            "<h1>Search results for {}</h1>{}<p>{} results</p>",
            escape(&response.query),
            form(&response.query),
            response.results.len()
        );
        if response.all_unresponsive {
            body.push_str("<p role=\"alert\">Search sources are temporarily unavailable.</p>");
        }
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
        let link = |page: u64| {
            let mut serializer = url::form_urlencoded::Serializer::new(String::from("/search?"));
            for (k, v) in &params {
                serializer.append_pair(k, v);
            }
            serializer.append_pair("pageno", &page.to_string());
            escape(&serializer.finish())
        };
        body.push_str("<nav aria-label=\"Search pages\">");
        if request.page > 1 {
            body.push_str(&format!(
                "<a href=\"{}\">← Previous</a> ",
                link(request.page - 1)
            ));
        }
        if let Some(next) = request.page.checked_add(1) {
            body.push_str(&format!("<a href=\"{}\">Try next page →</a>", link(next)));
        }
        body.push_str("</nav>");
        self.document(&format!("Search results for {}", response.query), &body)
    }
}
