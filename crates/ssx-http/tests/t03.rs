//! Real-process T03 HTTP acceptance; only fixture inputs are synthetic.
use ssx_http::{
    HtmlRenderer,
    portal::{PublicPortal, SCRIPT, STYLE, content_markup},
};
use ssx_json::Value;
use std::fs;
#[path = "support/t03_peer.rs"]
mod peer;
use peer::{Process, params, yaml};
#[test]
fn static_assets_and_highlight_boundary_preserve_safe_progressive_rendering() {
    let html = PublicPortal::default().landing();
    assert!(html.len() <= 50 * 1024);
    assert!(STYLE.len() + SCRIPT.len() <= 30 * 1024);
    assert_eq!(html.matches("<script>").count(), 1);
    assert!(
        !html.contains("<script src=")
            && !html.contains("@import")
            && !html.contains("fonts.googleapis")
    );
    assert_eq!(
        content_markup(
            "before <STRONG>a <img src=x></strong> <em onclick=x>bad</em> <mark>x&y</mark>"
        ),
        "before <strong>a &lt;img src=x&gt;</strong> &lt;em onclick=x&gt;bad&lt;/em&gt; <mark>x&amp;y</mark>"
    );
    assert_eq!(
        content_markup("<strong>unclosed <em>safe</em>"),
        "&lt;strong&gt;unclosed <em>safe</em>"
    );
}

#[tokio::test]
async fn inherited_scope_and_date_journeys_cover_all_16_literal_assertions() {
    let mut p = Process::start("inherited", "contract");
    let (status, body) = p
        .get(
            "/search?q=test&categories=general&pageno=2&time_range=month",
            "inherited-scope",
        )
        .await;
    assert_eq!(status, 200);
    for literal in [
        "Scope and filters",
        "name=\"categories\"",
        "value=\"general\"",
        "Past month",
        "← Previous",
        "Try next page →",
        "Why this result appeared",
        "Cross-source presence",
        "it is not confidence",
        "unsupported for this result",
        "structurally eligible",
    ] {
        assert!(body.contains(literal), "missing source assertion {literal}");
    }
    let (status, body) = p
        .get(
            "/search?q=climate&engines=openalex&date_from=2024-02-29",
            "inherited-date",
        )
        .await;
    assert_eq!(status, 200);
    for literal in [
        "name=\"date_from\" value=\"2024-02-29\"",
        "date_from=2024-02-29",
        "published from (2024-02-29): enforced",
    ] {
        assert!(body.contains(literal), "missing source assertion {literal}");
    }
    let source: Value = ssx_json::from_str(include_str!(
        "../../../docs/rust-rewrite/t02/original-bindings.json"
    ))
    .unwrap();
    let nodes = source["bindings"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|b| b["owner"] == "T03")
        .collect::<Vec<_>>();
    assert_eq!(nodes.len(), 2);
    assert_eq!(
        nodes
            .iter()
            .map(|b| b["assertions"].as_array().unwrap().len())
            .sum::<usize>(),
        16
    );
    fs::write(
        p.out.join("source-assertions.json"),
        ssx_json::to_string(&nodes).unwrap(),
    )
    .unwrap();
    assert_eq!(p.calls(), 3);
    p.stop().await;
}

#[tokio::test]
async fn browser_enforcement_uses_capture_without_changing_machine_contract() {
    let mut p = Process::start("metadata", "contract");
    let query = "q=climate&engines=openalex&date_from=2024-02-29&date_to=2024-03-01&language=fr&pageno=18446744073709551617&safesearch=1";
    let (status, html) = p.get(&format!("/search?{query}"), "metadata-html").await;
    assert_eq!(status, 200);
    let (_, body) = p
        .get(&format!("/search?{query}&format=json"), "metadata-json")
        .await;
    let json: Value = ssx_json::from_str(&body).unwrap();
    assert!(json["meta"].get("enforcement").is_none());
    let (_, body) = p
        .get(&format!("/search?{query}&format=yaml"), "metadata-yaml")
        .await;
    let yaml = yaml(&body);
    assert!(yaml["meta"].get("enforcement").is_none());
    for (field, status) in [
        ("language", "unsupported"),
        ("date_from", "enforced"),
        ("date_to", "enforced"),
    ] {
        assert!(html.contains(&format!("data-filter=\"{field}\" data-status=\"{status}\"")));
    }
    for literal in [
        "name=\"date_from\" value=\"2024-02-29\"",
        "name=\"date_to\" value=\"2024-03-01\"",
        "name=\"pageno\" value=\"1\"",
        "pageno=18446744073709551616",
        "pageno=18446744073709551618",
        "date_from=2024-02-29",
        "date_to=2024-03-01",
        "language=fr",
        "format=json",
        "format=yaml",
    ] {
        assert!(html.contains(literal), "missing state {literal}");
    }
    assert_eq!(p.calls(), 3);
    p.stop().await;
}

#[tokio::test]
async fn malicious_optional_empty_partial_and_unavailable_states_are_truthful() {
    let mut p = Process::start("states", "contract");
    for (query, wanted) in [
        ("malicious", 200),
        ("missing", 200),
        ("empty", 200),
        ("partial", 200),
        ("all-failed", 503),
        ("long", 200),
        ("groups", 200),
    ] {
        let (status, body) = p
            .get(
                &format!("/search?q={query}&categories=general"),
                &format!("state-{query}"),
            )
            .await;
        assert_eq!(status, wanted);
        assert_eq!(body.matches("<script>").count(), 1);
        assert!(
            !body.contains("href=\"javascript:")
                && !body.contains("<svg onload")
                && !body.contains("<img src=x")
        );
        match query {
            "malicious" => {
                assert!(body.contains("&lt;img src=x onerror=alert(1)&gt;"));
                assert!(body.contains("<strong>safe match</strong>"));
                assert!(!body.contains("class=\"result-link\""));
            }
            "missing" => assert!(!body.contains("<time ")),
            "empty" => {
                assert!(body.contains("No results found") && body.contains("broader query"));
                assert!(!body.contains("Try next page →"));
            }
            "partial" => assert!(
                body.contains("Some sources could not answer")
                    && body.contains("Beta &lt;source&gt;")
            ),
            "all-failed" => {
                assert!(
                    body.contains("No configured source returned a result")
                        && body.contains("role=\"alert\"")
                );
                assert!(!body.contains("Try next page →"));
            }
            "groups" => assert!(
                body.contains("CVE-2026-12345")
                    && body.contains("Source-reported conflicts")
                    && body.contains("#result-2")
            ),
            _ => {}
        }
    }
    p.stop().await;
}

#[tokio::test]
async fn unavailable_machine_format_keeps_the_legacy_unknown_scope_summary() {
    let mut p = Process::start("unavailable-format", "unavailable");
    let (status, html) = p
        .get(
            "/search?q=test&categories=general&format=html",
            "unavailable-html",
        )
        .await;
    assert_eq!(status, 503);
    assert!(html.contains("Scope and filters"));
    assert!(html.contains("0 of 2 sources answered"));
    assert_eq!(p.calls(), 0);

    let (status, body) = p
        .get(
            "/search?q=test&categories=general&format=yaml",
            "unavailable-yaml",
        )
        .await;
    assert_eq!(status, 503);
    let normalized = body
        .lines()
        .map(|line| {
            if line.starts_with("  query_id: ") {
                "  query_id: <opaque>"
            } else {
                line
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(
        normalized,
        concat!(
            "query: test\n",
            "meta:\n",
            "  response_time_ms: 0\n",
            "  cached: false\n",
            "  partial: false\n",
            "  deadline_exceeded: false\n",
            "  query_id: <opaque>\n",
            "  advisories: []\n",
            "  engine_count: 0\n",
            "  responsive: 0\n",
            "results: []\n",
            "---\n",
            "## Results Summary\n\n",
            "**? engines responded** of ? active. 0 results returned in 0.0s.\n\n",
            "For **test**, the top results cover:\n\n",
            "> all (no engines available)."
        )
    );
    assert_eq!(p.calls(), 0);
    p.stop().await;
}

#[tokio::test]
async fn invalid_and_sensitive_submissions_keep_only_escaped_public_fields_without_dispatch() {
    let mut p = Process::start("denials", "contract");
    for (id, query, status, field) in [
        (
            "strict",
            params(&[
                ("q", "<query>"),
                ("safesearch", "2"),
                ("api_key", "never-echo-this-secret"),
            ]),
            400,
            "safesearch",
        ),
        (
            "date",
            params(&[("q", "repair"), ("date_from", "<img src=x>")]),
            400,
            "date_from",
        ),
        (
            "restricted",
            params(&[("q", "repair"), ("engines", "hibp")]),
            403,
            "engines",
        ),
        (
            "page",
            params(&[("q", "retained"), ("pageno", "1.5")]),
            400,
            "",
        ),
    ] {
        let (actual, body) = p
            .get(&format!("/search?{query}"), &format!("deny-{id}"))
            .await;
        assert_eq!(actual, status);
        assert!(body.contains("class=\"search-input\""));
        assert!(!body.contains("never-echo-this-secret"));
        if !field.is_empty() {
            assert!(body.contains(&format!("data-invalid-field=\"{field}\"")));
        }
        if id == "date" {
            assert!(body.contains("type=\"text\" value=\"&lt;img src=x&gt;\""));
        }
        if id == "strict" {
            assert!(body.contains("value=\"&lt;query&gt;\""));
        }
    }
    assert_eq!(p.calls(), 0);
    p.stop().await;
}

#[tokio::test]
async fn cached_rate_limited_and_disabled_format_errors_retain_query_and_join() {
    for (scenario, wanted) in [
        ("cached-error", 503),
        ("rate-limited", 429),
        ("html-disabled", 403),
    ] {
        let mut p = Process::start(scenario, scenario);
        let (status, body) = p
            .get("/search?q=keep-me&categories=general", "error-recovery")
            .await;
        assert_eq!(status, wanted);
        assert!(
            body.contains("value=\"keep-me\"")
                && body.contains("value=\"general\"")
                && body.contains("role=\"alert\"")
        );
        assert_eq!(p.calls(), 0);
        p.stop().await;
    }
}

#[tokio::test]
async fn exact_page_limit_keeps_current_previous_and_disables_only_unadmitted_next() {
    let mut p = Process::start("page-limit", "contract");
    let page = "9".repeat(4300);
    let previous = format!("{}8", "9".repeat(4299));
    let (status, body) = p
        .get(
            &format!("/search?q=boundary&categories=general&pageno={page}"),
            "page-limit",
        )
        .await;
    assert_eq!(status, 200);
    assert!(body.contains(&format!("aria-current=\"page\">Page {page}")));
    assert!(body.contains(&format!("pageno={previous}")));
    assert!(body.contains("aria-disabled=\"true\">Next page unavailable (page number limit)"));
    assert!(!body.contains(&format!("pageno=1{}", "0".repeat(4300))));
    p.stop().await;
}

#[tokio::test]
async fn public_routes_ignore_opaque_cookie_bytes() {
    peer::public_cookie_equivalence("__Host-slopsearx_session=opaque-cookie-control").await;
}

#[tokio::test]
async fn blank_optional_html_bounds_are_absent_and_machine_blanks_stay_rejected() {
    let mut p = Process::start("blank-bounds", "contract");
    for method in ["GET", "POST"] {
        for format in ["html", "json", "yaml"] {
            for (case, from, to, selection, engines) in [
                ("both-empty", "", "", ("categories", "general"), 2),
                ("from-only", "2024-02-29", "", ("engines", "openalex"), 1),
                ("to-only", "", "2024-03-01", ("engines", "openalex"), 1),
            ] {
                let fields = params(&[
                    ("q", "blank-bounds"),
                    ("format", format),
                    selection,
                    ("date_from", from),
                    ("date_to", to),
                    ("time_range", ""),
                    ("safesearch", "0"),
                    ("pageno", "1"),
                ]);
                let target = if method == "GET" {
                    format!("/search?{fields}")
                } else {
                    "/search".into()
                };
                let before = p.calls();
                let (status, body) = p
                    .request(
                        method,
                        &target,
                        (method == "POST").then_some(fields.as_str()),
                        None,
                        &format!("blank-{method}-{format}-{case}"),
                    )
                    .await;
                if format == "html" {
                    assert_eq!(status, 200);
                    assert_eq!(p.calls() - before, engines);
                    if case == "both-empty" {
                        assert!(
                            !body.contains("published from (")
                                && !body.contains("published through (")
                        );
                    }
                } else {
                    assert_eq!(status, 400);
                    assert_eq!(p.calls(), before);
                }
            }
        }
    }
    p.stop().await;
}

#[tokio::test]
async fn landing_native_form_submits_query_and_scope_together_from_both_buttons() {
    let mut process = Process::start("landing-form", "contract");
    let (status, body) = process.get("/", "landing-form-source").await;
    assert_eq!(status, 200);
    let document = scraper::Html::parse_document(&body);
    let forms = scraper::Selector::parse("form").unwrap();
    let form = document.select(&forms).next().unwrap();
    assert_eq!(document.select(&forms).count(), 1);
    assert_eq!(form.value().attr("action"), Some("/search"));
    let controls = scraper::Selector::parse("input[name], select[name]").unwrap();
    let mut fields = Vec::new();
    for control in form.select(&controls) {
        let name = control.value().attr("name").unwrap();
        let value = match name {
            "q" => "first scoped query".to_owned(),
            "categories" => "science".to_owned(),
            "time_range" => "month".to_owned(),
            _ if control.value().name() == "select" => {
                let options = scraper::Selector::parse("option").unwrap();
                let available: Vec<_> = control
                    .select(&options)
                    .filter(|o| o.value().attr("disabled").is_none())
                    .collect();
                let option = available
                    .iter()
                    .find(|o| o.value().attr("selected").is_some())
                    .unwrap_or(&available[0]);
                option.value().attr("value").unwrap_or("").to_owned()
            }
            _ => control.value().attr("value").unwrap_or("").to_owned(),
        };
        fields.push((name.to_owned(), value));
    }
    let buttons = scraper::Selector::parse("button[type=submit]").unwrap();
    assert_eq!(form.select(&buttons).count(), 2);
    for (index, _) in form.select(&buttons).enumerate() {
        let borrowed: Vec<_> = fields
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        let target = format!("/search?{}", params(&borrowed));
        let before = process.calls();
        let (status, body) = process
            .get(&target, &format!("landing-submit-{index}"))
            .await;
        assert_eq!(status, 200);
        assert_eq!(
            process.calls() - before,
            1,
            "science scope dispatches only the fixture OpenAlex"
        );
        assert!(body.contains("first scoped query") && body.contains("Past month"));
    }
    process.stop().await;
}
