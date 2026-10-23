//! Parser-only A06 slice. No factory or HTTP decoding acceptance is implied.
use base64::Engine as _;
use ssx_domain::{AdapterResponse, EngineStatus};
use ssx_infra::adapters::arxiv::parse_feed;
use ssx_json::{Value, json};
use std::time::{Duration, Instant};
fn normalized(response: &AdapterResponse) -> Value {
    assert!(
        response.results.iter().all(|result| result.media.is_none()),
        "actual science result unexpectedly contains media"
    );
    json!({"results":response.results.iter().map(|r|json!({"url":r.url,"title":r.title,"content":r.content,"engine":r.engine,"engines":r.engines.iter().collect::<Vec<_>>(),"score":r.score,"position":r.position,"category":r.category,"published_date":r.published_date,"thumbnail":r.thumbnail,"img_src":r.img_src,"tier":r.tier,"media":null,"payload":r.payload,"work_group":r.work_group})).collect::<Vec<_>>(),"status":response.status,"error_message":response.error_message,"answers":response.answers,"corrections":response.corrections,"infoboxes":response.infoboxes,"synthetic":response.synthetic})
}
#[tokio::test]
async fn all_frozen_atom_parse_records_match_complete_source_outputs() {
    let mut counts = Vec::new();
    for source in [
        include_str!("../../../docs/rust-rewrite/a06/source-xml.json"),
        include_str!("../../../docs/rust-rewrite/a06/source-initial.json"),
        include_str!("../../../docs/rust-rewrite/a06/source-regressions.json"),
        include_str!("../../../docs/rust-rewrite/a06/source-shared.json"),
    ] {
        let corpus: Value = ssx_json::from_str(source).unwrap();
        let mut count = 0;
        for record in corpus["records"].as_array().unwrap() {
            // Only actual parse paths: transport/error/query-only obligations
            // remain assigned to the later full adapter implementation.
            if record["engine"] != json!("arxiv") || record["output"]["status"] != json!("ok") {
                continue;
            }
            let reply = record["responses"].as_array().unwrap().last().unwrap();
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(reply["body_b64"].as_str().unwrap())
                .unwrap();
            let text = std::str::from_utf8(&bytes).expect(
                "these frozen parse inputs are UTF8; this is not HTTPX codec implementation",
            );
            let out = parse_feed(text, Some(Instant::now() + Duration::from_secs(10))).await;
            assert_eq!(normalized(&out), record["output"], "{}", record["id"]);
            count += 1;
        }
        counts.push(count);
    }
    assert_eq!(
        counts,
        vec![24, 16, 10, 1],
        "fixed parser-only source inventory"
    );
}
fn feed(contents: &str) -> String {
    format!(r#"<feed xmlns="http://www.w3.org/2005/Atom"><entry>{contents}</entry></feed>"#)
}
#[tokio::test]
async fn namespace_entities_text_and_whole_document_failure() {
    let xml = feed("<title>A&#13;B\r\nC</title><summary>x</summary>");
    assert_eq!(parse_feed(&xml, None).await.results[0].title, "A\rB\nC");
    let xml = r#"<feed xmlns="http:&#47;&#47;www.w3.org/2005/Atom"><entry><title>T</title></entry></feed>"#;
    assert_eq!(parse_feed(xml, None).await.results[0].title, "T");
    let xml = format!(
        r#"<!DOCTYPE feed [<!ENTITY a "A&amp;B"><!ENTITY b "&a; C">]>{}"#,
        feed("<title>&b;</title>")
    );
    assert_eq!(parse_feed(&xml, None).await.results[0].title, "A&B C");
    let xml = format!(
        r#"<!DOCTYPE feed [<!ENTITY a "Before<b>child</b>After">]>{}"#,
        feed("<title>&a;</title>")
    );
    assert_eq!(parse_feed(&xml, None).await.results[0].title, "Before");
    for xml in [
        format!("{}<bad/>", feed("<title>T</title>")),
        feed("<title>&#0;</title>"),
        feed("<title>&not_defined;</title>"),
        feed("<title a='1' a='2'>T</title>"),
        feed("<bad:tag/>"),
        feed("<1bad/>"),
        feed("<title><!-- bad -- comment --></title>"),
    ] {
        let out = parse_feed(&xml, None).await;
        assert_eq!(out.status, EngineStatus::Ok);
        assert!(out.results.is_empty(), "{xml}");
    }
}
#[tokio::test]
async fn source_valid_deep_many_nodes_and_entity_chains() {
    let nested = format!("{}{}", "<x>".repeat(1000), "</x>".repeat(1000));
    assert_eq!(parse_feed(&nested, None).await.status, EngineStatus::Ok);
    let many = format!("<x>{}</x>", "<y/>".repeat(100001));
    assert_eq!(parse_feed(&many, None).await.status, EngineStatus::Ok);
    let mut decl = String::from("<!ENTITY e0 'x'>");
    for i in 1..=64 {
        decl.push_str(&format!("<!ENTITY e{i} '&e{};'>", i - 1));
    }
    let xml = format!("<!DOCTYPE feed [{decl}]>{}", feed("<title>&e64;</title>"));
    assert_eq!(parse_feed(&xml, None).await.results[0].title, "x");
}
#[tokio::test]
async fn inherited_deadline_and_dropped_future_are_cooperative() {
    let xml = feed("<title>T</title>");
    assert_eq!(
        parse_feed(&xml, Some(Instant::now())).await.status,
        EngineStatus::Timeout
    );
    let large = feed(&format!("<summary>{}</summary>", "x".repeat(2_000_000)));
    assert!(
        tokio::time::timeout(Duration::from_millis(1), parse_feed(&large, None))
            .await
            .is_err()
    );
    let out = parse_feed(&large, Some(Instant::now() + Duration::from_millis(1))).await;
    assert_eq!(out.status, EngineStatus::Timeout);
}

#[tokio::test]
async fn source_dtd_defaults_parameters_and_numeric_entity_references() {
    let xml = "<!DOCTYPE feed [<!ATTLIST feed xmlns CDATA 'http://www.w3.org/2005/Atom'>]><feed><entry><title>T</title></entry></feed>";
    assert_eq!(parse_feed(xml, None).await.results[0].title, "T");
    let xml = "<!DOCTYPE feed [<!ENTITY a '&#38;b;'><!ENTITY b 'B'>]><feed xmlns='http://www.w3.org/2005/Atom'><entry><title>&a;</title></entry></feed>";
    assert_eq!(parse_feed(xml, None).await.results[0].title, "B");
    let xml = "<!DOCTYPE feed [<!ENTITY % p \"<!ENTITY a 'A'>\">%p;]><feed xmlns='http://www.w3.org/2005/Atom'><entry><title>&a;</title></entry></feed>";
    assert!(parse_feed(xml, None).await.results.is_empty());
    let xml = "<!DOCTYPE feed [<!ENTITY % p \"<!ATTLIST feed xmlns CDATA 'http://www.w3.org/2005/Atom'>\">%p;]><feed><entry><title>T</title></entry></feed>";
    assert!(parse_feed(xml, None).await.results.is_empty());
    for level in [18, 20] {
        let mut declarations = String::from("<!ENTITY e0 'x'>");
        for i in 1..=level {
            declarations.push_str(&format!("<!ENTITY e{i} '&e{};&e{};'>", i - 1, i - 1));
        }
        let xml = format!(
            "<!DOCTYPE feed [{declarations}]>{}",
            feed(&format!("<title>&e{level};</title>"))
        );
        let out = parse_feed(&xml, None).await;
        assert_eq!(out.status, EngineStatus::Ok);
        assert_eq!(out.results.len(), usize::from(level == 18));
    }
}

#[tokio::test]
async fn actual_source_xml_grammar_controls() {
    let corpus: Value = ssx_json::from_str(include_str!(
        "../../../docs/rust-rewrite/a06/source-xml-grammar.json"
    ))
    .unwrap();
    let mut differences = Vec::new();
    for record in corpus["records"].as_array().unwrap() {
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(record["responses"][0]["body_b64"].as_str().unwrap())
            .unwrap();
        let text = std::str::from_utf8(&bytes).unwrap();
        let got = normalized(&parse_feed(text, None).await);
        if got != record["output"] {
            differences.push(format!(
                "{} expected={} actual={}",
                record["id"],
                ssx_json::to_string(&record["output"]).unwrap(),
                ssx_json::to_string(&got).unwrap()
            ));
        }
    }
    assert_eq!(corpus["records"].as_array().unwrap().len(), 165);
    assert!(differences.is_empty(), "{}", differences.join("\n"));
}
