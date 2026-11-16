//! Native public format writers. Policy and acquisition belong to SearchService.
use crate::{formatter_metadata, projection};
use ssx_domain::{SearchResponse, SearchResult};
use ssx_json::{Value, json};

pub fn meta(response: &SearchResponse, advisories: &[Value]) -> Value {
    let statuses = response.engine_outcomes.iter().map(|o| (o.engine.clone(),json!({
        "results":o.result_count,"latency_ms":o.latency_ms.as_ref().map_or(0.,|v| (v.get()*10.).round_ties_even()/10.),"status":o.status
    }))).collect();
    let mut meta = json!({"response_time_ms":response.response_time_ms,"cached":response.cached,"query_id":format!("ssx-{}",response.query_id),"engine_status":Value::Object(statuses),"partial":response.partial,"deadline_exceeded":response.deadline_exceeded});
    if let Some(proof) = &response.date_filter {
        meta["date_filter"] = ssx_json::to_value(proof).expect("typed date proof");
    }
    if !advisories.is_empty() {
        meta["advisories"] = json!(advisories);
    }
    if !response.scope.jev_added_engines.is_empty() {
        meta["jev_routing"] = json!({"added_engines":response.scope.jev_added_engines,"scores":response.scope.jev_scores});
    }
    if !response.empty_engines.is_empty() {
        meta["empty_engines"] = json!(response.empty_engines);
    }
    meta
}
pub fn json_document(response: &SearchResponse, meta: Value) -> Value {
    let metadata = response
        .results
        .iter()
        .map(formatter_metadata::metadata)
        .collect::<Vec<_>>();
    let dto = projection::search_json(response, Some(meta.clone()), &metadata)
        .expect("one metadata per result");
    let mut value = ssx_json::to_value(dto).expect("typed HTTP DTO");
    value["engines"] = json!(
        response
            .engine_outcomes
            .iter()
            .map(|o| json!({"engine":o.engine,"results":o.result_count}))
            .collect::<Vec<_>>()
    );
    value
}
fn csv_cell(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.into()
    }
}
pub fn csv(results: &[SearchResult]) -> String {
    let mut output = String::from("title,url,content,engine,publishedDate\n");
    for r in results {
        output.push_str(
            &[
                r.title.as_str(),
                &r.url,
                &r.content,
                &r.engine,
                r.published_date.as_deref().unwrap_or(""),
            ]
            .map(csv_cell)
            .join(","),
        );
        output.push('\n');
    }
    output
}
fn xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
fn element(name: &str, value: &str) -> String {
    if value.is_empty() {
        format!("<{name} />")
    } else {
        format!("<{name}>{}</{name}>", xml(value))
    }
}
pub fn rss(results: &[SearchResult], query: &str) -> String {
    let mut out =
        String::from("<?xml version='1.0' encoding='utf-8'?>\n<rss version=\"2.0\"><channel>");
    out.push_str(&element("title", &format!("SlopSearX results for {query}")));
    out.push_str(&element(
        "description",
        &format!("Search results for {query}"),
    ));
    for r in results {
        out.push_str("<item>");
        for (k, v) in [
            ("title", r.title.as_str()),
            ("link", &r.url),
            ("description", &r.content),
        ] {
            out.push_str(&element(k, v));
        }
        if let Some(published) = r.published_date.as_ref().filter(|s| !s.is_empty()) {
            out.push_str(&element("pubDate", published));
        }
        out.push_str("</item>");
    }
    out.push_str("</channel></rss>");
    out
}
/// Python repr spelling used by the original YAML error contract.
pub fn repr(input: &str) -> String {
    let quote = if input.contains('\'') && !input.contains('"') {
        '"'
    } else {
        '\''
    };
    let mut out = String::from(quote);
    let repr_quote = if quote == '\'' {
        ssx_domain::storage::PythonReprQuote::Single
    } else {
        ssx_domain::storage::PythonReprQuote::Double
    };
    for c in input.chars() {
        out.push_str(ssx_domain::storage::python_repr_character(c, repr_quote).as_str());
    }
    out.push(quote);
    out
}
pub fn python_repr(v: &Value) -> String {
    match v {
        Value::String(s) => repr(s),
        Value::Bool(b) => if *b { "True" } else { "False" }.into(),
        Value::Null => "None".into(),
        Value::Array(a) => format!(
            "[{}]",
            a.iter().map(python_repr).collect::<Vec<_>>().join(", ")
        ),
        Value::Object(o) => format!(
            "{{{}}}",
            o.iter()
                .map(|(k, v)| format!("{}: {}", repr(k), python_repr(v)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        _ => v.to_string(),
    }
}
fn scalar(v: &Value) -> String {
    match v {
        Value::Null => "null".into(),
        Value::String(s) => {
            let lower = s.to_lowercase();
            let plain = !s.is_empty()
                && !s.starts_with([
                    '-', '?', ':', ',', '[', ']', '{', '}', '#', '&', '*', '!', '|', '>', '\'',
                    '"', '%', '@', '`',
                ])
                && s.trim() == s
                && !s.contains(['\n', '\r'])
                && !s.contains(": ")
                && !s.contains(" #")
                && !s.ends_with(':')
                && ![
                    "null", "~", "true", "false", "yes", "no", "on", "off", ".nan", ".inf",
                    "-.inf", "+.inf",
                ]
                .contains(&lower.as_str())
                && !s
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_ascii_digit() || matches!(c, '+' | '-' | '.'))
                && !s.chars().any(char::is_control);
            if plain {
                s.clone()
            } else if !s.chars().any(char::is_control) {
                format!("'{}'", s.replace('\'', "''"))
            } else {
                ssx_json::to_string(v).expect("string")
            }
        }
        Value::Array(a) if a.is_empty() => "[]".into(),
        Value::Object(o) if o.is_empty() => "{}".into(),
        _ => v.to_string(),
    }
}
fn complex(v: &Value) -> bool {
    matches!(v,Value::Object(o) if !o.is_empty()) || matches!(v,Value::Array(a) if !a.is_empty())
}
fn scalar_line(value: &Value, indent: usize, out: &mut String) {
    let text = scalar(value);
    let mut chunks = text.split_inclusive(' ').peekable();
    while let Some(chunk) = chunks.next() {
        let space = chunk.ends_with(' ');
        let word = if space {
            &chunk[..chunk.len() - 1]
        } else {
            chunk
        };
        out.push_str(word);
        if space {
            let column = out.rsplit('\n').next().unwrap_or("").chars().count();
            if column > 80
                && chunks.peek().is_some_and(|next| !next.starts_with(' '))
                && !word.is_empty()
            {
                out.push('\n');
                out.push_str(&" ".repeat(indent + 2));
            } else {
                out.push(' ');
            }
        }
    }
}
fn write_yaml(value: &Value, indent: usize, out: &mut String) {
    let pad = " ".repeat(indent);
    match value {
        Value::Object(map) => {
            for (key, v) in map {
                out.push_str(&pad);
                out.push_str(&scalar(&Value::String(key.clone())));
                out.push(':');
                if complex(v) {
                    out.push('\n');
                    write_yaml(v, if v.is_array() { indent } else { indent + 2 }, out);
                } else {
                    out.push(' ');
                    scalar_line(v, indent, out);
                    out.push('\n');
                }
            }
        }
        Value::Array(list) => {
            for v in list {
                out.push_str(&pad);
                out.push('-');
                if let Value::Object(map) = v
                    && !map.is_empty()
                {
                    out.push(' ');
                    let mut nested = String::new();
                    write_yaml(v, indent + 2, &mut nested);
                    out.push_str(nested.trim_start());
                    continue;
                }
                if complex(v) {
                    out.push('\n');
                    write_yaml(v, indent + 2, out);
                } else {
                    out.push(' ');
                    scalar_line(v, indent, out);
                    out.push('\n');
                }
            }
        }
        _ => {
            out.push_str(&pad);
            out.push_str(&scalar(value));
            out.push('\n');
        }
    }
}
pub fn yaml_document(response: &SearchResponse, meta: &Value) -> String {
    let mut selected = json!({});
    for (key, default) in [
        ("response_time_ms", json!(0)),
        ("cached", json!(false)),
        ("partial", json!(false)),
        ("deadline_exceeded", json!(false)),
        ("query_id", json!("")),
    ] {
        selected[key] = meta.get(key).cloned().unwrap_or(default);
    }
    for key in ["date_filter", "advisories", "jev_routing"] {
        if let Some(v) = meta.get(key) {
            selected[key] = v.clone();
        }
    }
    if meta.get("date_filter").is_some() {
        selected["engine_status"] = meta["engine_status"].clone();
    }
    selected["engine_count"] = json!(response.engine_outcomes.len());
    let responsive = response
        .engine_outcomes
        .iter()
        .filter(|o| o.status == "ok")
        .count();
    selected["responsive"] = json!(responsive);
    let document = json!({"query":response.query,"meta":selected,"results":response.results.iter().map(|r|ssx_json::to_value(projection::result_yaml(r)).expect("typed YAML DTO")).collect::<Vec<_>>()});
    let mut output = String::new();
    write_yaml(&document, 0, &mut output);
    output.push_str("---\n## Results Summary\n\n");
    let count = |n: usize| if n == 0 { "?".into() } else { n.to_string() };
    output.push_str(&format!("**{} engines responded** of {} active. {} results returned in {:.1}s.\n\nFor **{}**, the top results cover:\n",count(responsive),count(response.scope.selected_engines.len()),response.results.len(),response.response_time_ms as f64/1000.,response.query));
    for r in response.results.iter().take(5) {
        let snippet = r
            .content
            .chars()
            .take(120)
            .collect::<String>()
            .trim()
            .replace('\n', " ");
        output.push_str("- ");
        output.push_str(if snippet.is_empty() {
            &r.title
        } else {
            &snippet
        });
        output.push('\n');
    }
    let mut unresponsive = response
        .engine_outcomes
        .iter()
        .filter(|o| o.status != "ok")
        .map(|o| {
            format!(
                "{} ({})",
                o.engine,
                o.message.as_deref().unwrap_or(&o.status)
            )
        })
        .collect::<Vec<_>>();
    if unresponsive.is_empty() && response.all_unresponsive && response.engine_outcomes.is_empty() {
        unresponsive.push("all (no engines available)".into());
    }
    if !unresponsive.is_empty() {
        output.push_str(&format!("\n> {}.\n", unresponsive.join(", ")));
    }
    output
}
