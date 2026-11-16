//! arXiv Atom extraction from already HTTPX-compatible decoded scalar text.
//! HTTP, redirects, query construction and factory activation remain A06 work.
use quick_xml::{Reader, events::Event};
use ssx_domain::{AdapterResponse, EngineStatus, SearchResult};
use ssx_json::{Map, Value};
use std::{collections::BTreeMap, time::Instant};

const ATOM: &str = "http://www.w3.org/2005/Atom";
const ARXIV: &str = "http://arxiv.org/schemas/atom";
/// F08 admits 32MiB decoded bytes. HTTPX replacement decoding may expand each
/// byte into a three-byte scalar; allow four UTF8 bytes per source code point.
pub const MAX_INPUT_BYTES: usize = 128 * 1024 * 1024;
pub const MAX_CODEPOINTS: usize = 32 * 1024 * 1024;
pub const MAX_TREE_BYTES: usize = 128 * 1024 * 1024;

#[derive(Debug)]
pub(super) enum Failure {
    Syntax,
    Limit(&'static str),
    Deadline,
}
type Result<T> = std::result::Result<T, Failure>;
pub(super) async fn checkpoint(deadline: Option<Instant>) -> Result<()> {
    if deadline.is_some_and(|d| Instant::now() >= d) {
        return Err(Failure::Deadline);
    }
    tokio::task::yield_now().await;
    if deadline.is_some_and(|d| Instant::now() >= d) {
        return Err(Failure::Deadline);
    }
    Ok(())
}
pub(super) fn xml_char(c: char) -> bool {
    matches!(c, '\t'|'\n'|'\r'|'\u{20}'..='\u{d7ff}'|'\u{e000}'..='\u{fffd}'|'\u{10000}'..='\u{10ffff}')
}
fn ws(c: char) -> bool {
    c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c)
}
fn trim(s: &str) -> &str {
    s.trim_matches(ws)
}

/// Classified source parse outcome, with explicit native resource failures.
/// Malformed XML is a source OK-empty response, including errors after entries.
/// This future has no detached worker. `deadline` must be the operation deadline.
pub async fn parse_feed(text: &str, deadline: Option<Instant>) -> AdapterResponse {
    let result = parse(text, deadline).await;
    match result {
        Ok(results) => AdapterResponse {
            results,
            ..Default::default()
        },
        Err(Failure::Syntax) => AdapterResponse::default(),
        Err(Failure::Deadline) => AdapterResponse {
            status: EngineStatus::Timeout,
            ..Default::default()
        },
        Err(error) => AdapterResponse {
            status: EngineStatus::Error,
            error_message: Some(match error {
                Failure::Limit(kind) => format!("arXiv XML exceeds native {kind} limit"),
                _ => unreachable!(),
            }),
            ..Default::default()
        },
    }
}

#[derive(Default)]
struct Node {
    namespace: String,
    name: String,
    text: String,
    children: Vec<usize>,
}
impl Node {
    fn child<'a>(&self, nodes: &'a [Node], ns: &str, name: &str) -> Option<&'a Node> {
        self.children
            .iter()
            .map(|i| &nodes[*i])
            .find(|n| n.namespace == ns && n.name == name)
    }
    fn findtext(&self, nodes: &[Node], ns: &str, name: &str) -> Option<String> {
        self.child(nodes, ns, name).map(|n| n.text.clone())
    }
}
fn name_parts<'a>(
    name: &'a str,
    namespaces: &BTreeMap<String, String>,
    attribute: bool,
) -> Result<(String, &'a str)> {
    let (prefix, local) = name.split_once(':').unwrap_or(("", name));
    if name.starts_with(':')
        || local.is_empty()
        || local.contains(':')
        || !valid_name(local)
        || (!prefix.is_empty() && !valid_name(prefix))
    {
        return Err(Failure::Syntax);
    }
    let namespace = if attribute && prefix.is_empty() {
        String::new()
    } else {
        namespaces.get(prefix).cloned().unwrap_or_default()
    };
    if !prefix.is_empty() && namespace.is_empty() {
        return Err(Failure::Syntax);
    }
    Ok((namespace, local))
}
// XML1.0 Fifth Edition NameStartChar/NameChar, rather than Rust identifier rules.
pub(super) fn name_start(c: char) -> bool {
    matches!(c, ':'|'_'|'A'..='Z'|'a'..='z'|'\u{c0}'..='\u{d6}'|'\u{d8}'..='\u{f6}'|'\u{f8}'..='\u{2ff}'|'\u{370}'..='\u{37d}'|'\u{37f}'..='\u{1fff}'|'\u{200c}'..='\u{200d}'|'\u{2070}'..='\u{218f}'|'\u{2c00}'..='\u{2fef}'|'\u{3001}'..='\u{d7ff}'|'\u{f900}'..='\u{fdcf}'|'\u{fdf0}'..='\u{fffd}'|'\u{10000}'..='\u{effff}')
}
fn valid_name(s: &str) -> bool {
    let mut c = s.chars();
    c.next().is_some_and(name_start) && c.all(|c| {
        name_start(c)
            || matches!(c,'-'|'.'|'0'..='9'|'\u{b7}'|'\u{300}'..='\u{36f}'|'\u{203f}'..='\u{2040}')
    })
}

async fn parse(text: &str, deadline: Option<Instant>) -> Result<Vec<SearchResult>> {
    if text.len() > MAX_INPUT_BYTES {
        return Err(Failure::Limit("input byte"));
    }
    for (i, c) in text.chars().enumerate() {
        if i % 4096 == 0 {
            checkpoint(deadline).await?;
        }
        if i >= MAX_CODEPOINTS {
            return Err(Failure::Limit("input code point"));
        }
        if !xml_char(c) {
            return Err(Failure::Syntax);
        }
    }
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    if text.starts_with('\u{feff}') {
        return Err(Failure::Syntax);
    }
    let (expanded, dtd, _) = prepare(text, deadline).await?;
    let mut reader = Reader::from_str(&expanded);
    reader.config_mut().check_comments = true;
    let mut nodes: Vec<Node> = Vec::new();
    let mut stack: Vec<(usize, BTreeMap<String, String>)> = Vec::new();
    let mut root = None;
    let mut event_count = 0usize;
    let mut tree_bytes = 0usize;
    let mut namespace_bytes = 0usize;
    let mut declaration = false;
    loop {
        if event_count.is_multiple_of(128) {
            checkpoint(deadline).await?;
        }
        event_count += 1;
        match reader.read_event().map_err(|_| Failure::Syntax)? {
            event @ (Event::Start(_) | Event::Empty(_)) => {
                let empty = matches!(event, Event::Empty(_));
                let start = match event {
                    Event::Start(s) | Event::Empty(s) => s,
                    _ => unreachable!(),
                };

                let mut namespaces = stack.last().map(|(_, n)| n.clone()).unwrap_or_else(|| {
                    BTreeMap::from([("xml".into(), "http://www.w3.org/XML/1998/namespace".into())])
                });
                let mut attributes = Vec::new();
                let raw_name = start.name().as_ref().to_owned();
                let mut raw_attrs = Vec::new();
                for (aindex, attr) in start.attributes().enumerate() {
                    if aindex % 128 == 0 {
                        checkpoint(deadline).await?;
                    }
                    let attr = attr.map_err(|_| Failure::Syntax)?;
                    raw_attrs.push((attr.key.as_ref().to_owned(), attr.value.into_owned()));
                }
                if let Some(defaults) = dtd.attributes.get(&raw_name) {
                    for default in defaults {
                        if !raw_attrs.iter().any(|(k, _)| k == &default.name)
                            && let Some(value) = &default.value
                        {
                            raw_attrs.push((
                                default.name.clone(),
                                expand(value, &dtd.entities, text.len(), deadline).await?,
                            ));
                        }
                    }
                }
                for (aindex, (key, raw)) in raw_attrs.into_iter().enumerate() {
                    if aindex % 128 == 0 {
                        checkpoint(deadline).await?;
                    }
                    let raw = raw.replace("\r\n", " ").replace(['\r', '\n', '\t'], " ");
                    if raw.contains('<') {
                        return Err(Failure::Syntax);
                    }
                    let mut value = quick_xml::escape::unescape(&raw)
                        .map_err(|_| Failure::Syntax)?
                        .into_owned();
                    if !value.chars().all(xml_char) {
                        return Err(Failure::Syntax);
                    }
                    if dtd
                        .attributes
                        .get(&raw_name)
                        .is_some_and(|attrs| attrs.iter().any(|a| a.name == key && a.tokenized))
                    {
                        value = value
                            .split(' ')
                            .filter(|s| !s.is_empty())
                            .collect::<Vec<_>>()
                            .join(" ");
                    }
                    if key == "xmlns" || key.starts_with("xmlns:") {
                        let prefix = key.strip_prefix("xmlns:").unwrap_or("");
                        if (!prefix.is_empty() && !valid_name(prefix))
                            || prefix == "xmlns"
                            || (!prefix.is_empty() && value.is_empty())
                            || value == "http://www.w3.org/2000/xmlns/"
                            || (prefix == "xml")
                                != (value == "http://www.w3.org/XML/1998/namespace")
                        {
                            return Err(Failure::Syntax);
                        }
                        namespaces.insert(prefix.into(), value);
                    } else {
                        attributes.push(key);
                    }
                }
                let mut expanded_attrs = std::collections::BTreeSet::new();
                for attr in attributes {
                    let (ns, n) = name_parts(&attr, &namespaces, true)?;
                    if !expanded_attrs.insert((ns, n.to_owned())) {
                        return Err(Failure::Syntax);
                    }
                }
                let raw_name = start.name().as_ref().to_owned();
                let (ns, name) = name_parts(&raw_name, &namespaces, false)?;
                tree_bytes += std::mem::size_of::<Node>()
                    + ns.len()
                    + name.len()
                    + std::mem::size_of::<usize>();
                if tree_bytes > MAX_TREE_BYTES {
                    return Err(Failure::Limit("tree allocation byte (134217728)"));
                }
                let index = nodes.len();
                nodes.push(Node {
                    namespace: ns,
                    name: name.into(),
                    ..Default::default()
                });
                if let Some((parent, _)) = stack.last() {
                    nodes[*parent].children.push(index);
                } else if root.replace(index).is_some() {
                    return Err(Failure::Syntax);
                }
                if !empty {
                    namespace_bytes += namespaces
                        .iter()
                        .map(|(k, v)| k.len() + v.len() + 64)
                        .sum::<usize>();
                    if namespace_bytes > MAX_TREE_BYTES {
                        return Err(Failure::Limit("namespace allocation byte (134217728)"));
                    }
                    stack.push((index, namespaces));
                }
            }
            Event::End(_) => {
                let Some((_, namespaces)) = stack.pop() else {
                    return Err(Failure::Syntax);
                };
                namespace_bytes -= namespaces
                    .iter()
                    .map(|(k, v)| k.len() + v.len() + 64)
                    .sum::<usize>();
            }
            Event::Text(t) => {
                let value = t.xml_content(quick_xml::XmlVersion::Implicit1_0);
                if value.contains("]]>") {
                    return Err(Failure::Syntax);
                }
                append_text(&mut nodes, &stack, &value, deadline).await?;
            }
            Event::CData(t) => {
                if stack.is_empty() {
                    return Err(Failure::Syntax);
                }
                let value = t.xml_content(quick_xml::XmlVersion::Implicit1_0);
                append_text(&mut nodes, &stack, &value, deadline).await?;
            }
            Event::GeneralRef(r) => {
                let c = if let Some(c) = r.resolve_char_ref().map_err(|_| Failure::Syntax)? {
                    c.to_string()
                } else {
                    match r.as_ref() {
                        "amp" => "&",
                        "lt" => "<",
                        "gt" => ">",
                        "apos" => "'",
                        "quot" => "\"",
                        _ => return Err(Failure::Syntax),
                    }
                    .into()
                };
                if stack.is_empty() || !c.chars().all(xml_char) {
                    return Err(Failure::Syntax);
                }
                append_text(&mut nodes, &stack, &c, deadline).await?;
            }
            Event::Decl(d) => {
                super::arxiv_xml::validate_declaration(d.as_ref()).map_err(|_| Failure::Syntax)?;
                if declaration
                    || root.is_some()
                    || !(expanded.starts_with("<?xml") || expanded.starts_with("\u{feff}<?xml"))
                    || d.version().is_err()
                {
                    return Err(Failure::Syntax);
                }
                declaration = true;
            }
            Event::Comment(_) => {}
            Event::PI(p) => {
                super::arxiv_xml::validate_pi(p.as_ref()).map_err(|_| Failure::Syntax)?;
            }
            Event::DocType(_) => return Err(Failure::Syntax), // removed by bounded preparation
            Event::Eof => break,
        }
    }
    if !stack.is_empty() {
        return Err(Failure::Syntax);
    }
    let root = root.ok_or(Failure::Syntax)?;
    let mut results = Vec::new();
    let mut output_bytes = 0;
    for index in &nodes[root].children {
        checkpoint(deadline).await?;
        let entry = &nodes[*index];
        if entry.namespace != ATOM || entry.name != "entry" {
            continue;
        }
        let field = |name| entry.findtext(&nodes, ATOM, name).unwrap_or_default();
        let title = trim(&field("title")).to_owned();
        let summary = field("summary");
        let mut abstract_text = String::new();
        let mut pending = false;
        for (i, c) in summary.chars().enumerate() {
            if i % 4096 == 0 {
                checkpoint(deadline).await?;
            }
            if ws(c) {
                pending = !abstract_text.is_empty();
            } else {
                if pending {
                    abstract_text.push(' ');
                    pending = false;
                }
                abstract_text.push(c);
            }
        }
        let id = field("id");
        let id = trim(&id);
        let id = id.rsplit('/').next().unwrap_or(id);
        let mut data: Map<String, Value> = Map::new();
        if !id.is_empty() {
            data.insert("publication_id".into(), Value::String(id.into()));
            data.insert("arxiv_id".into(), Value::String(id.into()));
        }
        if let Some(v) = entry.findtext(&nodes, ARXIV, "doi") {
            data.insert("doi".into(), Value::String(v));
        }
        let mut authors = Vec::new();
        for (i, child) in entry.children.iter().enumerate() {
            if i % 128 == 0 {
                checkpoint(deadline).await?;
            }
            let a = &nodes[*child];
            if a.namespace == ATOM && a.name == "author" {
                for (i, child) in a.children.iter().enumerate() {
                    if i % 128 == 0 {
                        checkpoint(deadline).await?;
                    }
                    let n = &nodes[*child];
                    if n.namespace == ATOM && n.name == "name" && !trim(&n.text).is_empty() {
                        authors.push(Value::String(trim(&n.text).into()));
                    }
                }
            }
        }
        if !authors.is_empty() {
            data.insert("authors".into(), Value::Array(authors));
        }
        for (source, target, ns) in [
            ("journal_ref", "journal", ARXIV),
            ("comment", "comments", ARXIV),
            ("updated", "revision_date", ATOM),
        ] {
            if let Some(v) = entry.findtext(&nodes, ns, source) {
                data.insert(target.into(), Value::String(v));
            }
        }
        if !abstract_text.is_empty() {
            data.insert("abstract".into(), Value::String(abstract_text.clone()));
        }
        let fields = data.keys().cloned().collect::<Vec<_>>();
        let payload = Some(
            ssx_json::json!({"domain":"science","type":"publication","schema_version":1,"data":data,"provenance":{"engine":"arxiv","adapter_fields":fields,"normalized_fields":[],"inferred_fields":[]}}),
        );
        let mut content = abstract_text.chars().take(300).collect::<String>();
        if abstract_text.chars().count() > 300 {
            content.push('…');
        }
        let published_date = entry
            .child(&nodes, ATOM, "published")
            .filter(|n| !n.text.is_empty())
            .map(|n| trim(&n.text).to_owned());
        let row = SearchResult {
            url: format!("https://arxiv.org/abs/{id}"),
            title,
            content,
            engine: "arxiv".into(),
            position: results.len() as i64 + 1,
            published_date,
            payload,
            ..Default::default()
        };
        output_bytes += row.url.len()
            + row.title.len()
            + row.content.len()
            + row.published_date.as_ref().map_or(0, String::len)
            + row
                .payload
                .as_ref()
                .map_or(0, |p| ssx_json::to_vec(p).expect("finite payload").len());
        if output_bytes > MAX_INPUT_BYTES {
            return Err(Failure::Limit("output byte"));
        }
        results.push(row);
    }
    checkpoint(deadline).await?;
    Ok(results)
}
async fn append_text(
    nodes: &mut [Node],
    stack: &[(usize, BTreeMap<String, String>)],
    value: &str,
    deadline: Option<Instant>,
) -> Result<()> {
    if let Some((i, _)) = stack.last() {
        if nodes[*i].children.is_empty() {
            for (n, c) in value.chars().enumerate() {
                if n % 4096 == 0 {
                    checkpoint(deadline).await?;
                }
                nodes[*i].text.push(c);
            }
        }
    } else if !value.chars().all(|c| matches!(c, ' ' | '\t' | '\r' | '\n')) {
        return Err(Failure::Syntax);
    }
    Ok(())
}

/// DTD general entities are handled locally; external entities are never loaded.
/// Default attributes/parameter entities require additional source observations.
async fn prepare(
    text: &str,
    deadline: Option<Instant>,
) -> Result<(String, Dtd, super::arxiv_accounting::Accounting)> {
    use super::arxiv_accounting::{Accounting, Context, Mode};
    let mut accounting = Accounting::default();
    let mut dtd = Dtd::default();
    let mut reader = Reader::from_str(text);
    let mut without = String::new();
    let mut previous = 0;
    let mut doctype = false;
    let mut root = false;
    let mut count = 0;
    loop {
        if count % 128 == 0 {
            checkpoint(deadline).await?;
        }
        count += 1;
        let event = reader.read_event().map_err(|_| Failure::Syntax)?;
        let end = reader.buffer_position() as usize;
        match event {
            Event::Decl(d) => {
                if previous != 0 {
                    return Err(Failure::Syntax);
                }
                super::arxiv_xml::validate_declaration(d.as_ref()).map_err(|_| Failure::Syntax)?;
                dtd.standalone = d.standalone().is_some_and(|s| s.is_ok_and(|v| v == "yes"));
                accounting.add(Mode::Direct, end - previous, true)?;
                without.push_str(&text[previous..end]);
            }
            Event::DocType(d) => {
                if doctype || root {
                    return Err(Failure::Syntax);
                }
                doctype = true;
                let value = d.as_ref();
                let raw = &text[previous..end];
                let offset = raw.find(value).ok_or(Failure::Syntax)?;
                let mut consumed = 0usize;
                parse_dtd(
                    value,
                    &mut dtd,
                    deadline,
                    &mut accounting,
                    offset,
                    &mut consumed,
                )
                .await?;
                accounting.add(
                    Mode::Direct,
                    raw.len().checked_sub(consumed).ok_or(Failure::Syntax)?,
                    true,
                )?;
            }
            Event::Eof => break,
            _ => {
                match &event {
                    Event::Text(_) => {
                        super::arxiv_accounting::text_tokens(
                            &text[previous..end],
                            Mode::Direct,
                            &mut accounting,
                            deadline,
                        )
                        .await?
                    }
                    Event::CData(_) => {
                        accounting.add(Mode::Direct, 9, true)?;
                        super::arxiv_accounting::text_tokens(
                            &text[previous + 9..end - 3],
                            Mode::Direct,
                            &mut accounting,
                            deadline,
                        )
                        .await?;
                        accounting.add(Mode::Direct, 3, true)?;
                    }
                    _ => accounting.add(Mode::Direct, end - previous, true)?,
                }
                match &event {
                    Event::Start(s) | Event::Empty(s) => {
                        super::arxiv_accounting::start_attributes(
                            s,
                            &dtd.entities,
                            Mode::Direct,
                            &mut accounting,
                            deadline,
                        )
                        .await?
                    }
                    Event::GeneralRef(_) => {
                        let raw = &text[previous..end];
                        super::arxiv_accounting::fragment(
                            raw,
                            &dtd.entities,
                            Mode::None,
                            Context::Content,
                            &mut accounting,
                            deadline,
                        )
                        .await?;
                    }
                    _ => {}
                }
                root |= matches!(event, Event::Start(_) | Event::Empty(_));
                without.push_str(&text[previous..end]);
            }
        }
        previous = end;
    }
    let expanded = expand(&without, &dtd.entities, text.len(), deadline).await?;
    Ok((expanded, dtd, accounting))
}
#[derive(Default)]
struct Dtd {
    standalone: bool,
    entities: BTreeMap<String, Option<String>>,
    attributes: BTreeMap<String, Vec<AttributeDecl>>,
}
struct AttributeDecl {
    name: String,
    tokenized: bool,
    value: Option<String>,
}
fn declaration_end(s: &str) -> Result<usize> {
    let mut quote = None;
    for (i, c) in s.char_indices() {
        if let Some(q) = quote {
            if c == q {
                quote = None;
            }
        } else if matches!(c, '\'' | '"') {
            quote = Some(c);
        } else if c == '>' {
            return Ok(i);
        }
    }
    Err(Failure::Syntax)
}
fn word(s: &mut &str) -> Result<String> {
    *s = s.trim_start_matches([' ', '\t', '\r', '\n']);
    let end = s.find(|c: char| c.is_ascii_whitespace()).unwrap_or(s.len());
    if end == 0 {
        return Err(Failure::Syntax);
    }
    let out = s[..end].to_owned();
    *s = &s[end..];
    Ok(out)
}
fn quoted(s: &mut &str) -> Result<String> {
    *s = s.trim_start_matches([' ', '\t', '\r', '\n']);
    let quote = s.chars().next().ok_or(Failure::Syntax)?;
    if !matches!(quote, '\'' | '"') {
        return Err(Failure::Syntax);
    }
    let end = s[1..].find(quote).ok_or(Failure::Syntax)? + 1;
    let out = s[1..end].to_owned();
    *s = &s[end + 1..];
    Ok(out)
}
async fn parse_dtd(
    text: &str,
    dtd: &mut Dtd,
    deadline: Option<Instant>,
    accounting: &mut super::arxiv_accounting::Accounting,
    offset: usize,
    consumed: &mut usize,
) -> Result<()> {
    super::arxiv_xml::validate_dtd(text, deadline)
        .await
        .map_err(|_| {
            if deadline.is_some_and(|d| Instant::now() >= d) {
                Failure::Deadline
            } else {
                Failure::Syntax
            }
        })?;
    let mut quote = None;
    let mut at = None;
    for (i, c) in text.char_indices() {
        if let Some(q) = quote {
            if c == q {
                quote = None;
            }
        } else if matches!(c, '\'' | '"') {
            quote = Some(c);
        } else if c == '[' {
            at = Some(i);
            break;
        }
    }
    let Some(at) = at else {
        return Ok(());
    };
    let subset = text[at + 1..].strip_suffix(']').ok_or(Failure::Syntax)?;
    let mut s = subset.trim();
    let mut count = 0;
    let mut process = true;
    while !s.is_empty() {
        if count % 128 == 0 {
            checkpoint(deadline).await?;
        }
        count += 1;
        if let Some(rest) = s.strip_prefix("<!--") {
            s = rest
                .split_once("-->")
                .ok_or(Failure::Syntax)?
                .1
                .trim_start();
            continue;
        }
        if let Some(rest) = s.strip_prefix("<?") {
            s = rest.split_once("?>").ok_or(Failure::Syntax)?.1.trim_start();
            continue;
        }
        // Expat's default non-validating parser does not read parameter entities.
        // Encountering a reference also stops subsequent declaration processing.
        if s.starts_with('%') {
            let end = s.find(';').ok_or(Failure::Syntax)?;
            if !valid_name(&s[1..end]) {
                return Err(Failure::Syntax);
            }
            process = dtd.standalone;
            s = s[end + 1..].trim_start();
            continue;
        }
        let end = declaration_end(s)?;
        let declaration = &s[..end];
        s = s[end + 1..].trim_start();
        if let Some(rest) = declaration.strip_prefix("<!ENTITY") {
            if !rest.starts_with(char::is_whitespace) {
                return Err(Failure::Syntax);
            }
            let mut rest = rest.trim_start();
            let parameter = rest.starts_with('%');
            if parameter {
                rest = rest[1..].trim_start();
            }
            let name = word(&mut rest)?;
            if !valid_name(&name) {
                return Err(Failure::Syntax);
            }
            rest = rest.trim_start();
            let value = if rest.starts_with(['\'', '"']) {
                let value = quoted(&mut rest)?;
                if !rest.trim().is_empty() || value.contains('%') {
                    return Err(Failure::Syntax);
                }
                Some(numeric_entity_value(&value, deadline).await?)
            } else if rest.starts_with("SYSTEM") || rest.starts_with("PUBLIC") {
                let public = word(&mut rest)? == "PUBLIC";
                let _ = quoted(&mut rest)?;
                if public {
                    let _ = quoted(&mut rest)?;
                }
                if !rest.trim().is_empty() {
                    let ndata = word(&mut rest)?;
                    let n = word(&mut rest)?;
                    if parameter || ndata != "NDATA" || !valid_name(&n) || !rest.trim().is_empty() {
                        return Err(Failure::Syntax);
                    }
                }
                None
            } else {
                return Err(Failure::Syntax);
            };
            if !parameter && process {
                dtd.entities.entry(name).or_insert(value);
            }
        } else if let Some(mut rest) = declaration.strip_prefix("<!ATTLIST") {
            let element = word(&mut rest)?;
            if !valid_name(&element) {
                return Err(Failure::Syntax);
            }
            while !rest.trim().is_empty() {
                let name = word(&mut rest)?;
                if !valid_name(&name) {
                    return Err(Failure::Syntax);
                }
                rest = rest.trim_start();
                let tokenized = if rest.starts_with('(') {
                    let end = rest.find(')').ok_or(Failure::Syntax)?;
                    rest = &rest[end + 1..];
                    true
                } else {
                    let kind = word(&mut rest)?;
                    if kind == "NOTATION" {
                        rest = rest.trim_start();
                        if !rest.starts_with('(') {
                            return Err(Failure::Syntax);
                        }
                        let end = rest.find(')').ok_or(Failure::Syntax)?;
                        rest = &rest[end + 1..];
                    } else if !matches!(
                        kind.as_str(),
                        "CDATA"
                            | "ID"
                            | "IDREF"
                            | "IDREFS"
                            | "ENTITY"
                            | "ENTITIES"
                            | "NMTOKEN"
                            | "NMTOKENS"
                    ) {
                        return Err(Failure::Syntax);
                    }
                    kind != "CDATA"
                };
                rest = rest.trim_start();
                let value = if rest.starts_with("#IMPLIED") {
                    rest = &rest[8..];
                    None
                } else if rest.starts_with("#REQUIRED") {
                    rest = &rest[9..];
                    None
                } else {
                    if rest.starts_with("#FIXED") {
                        rest = &rest[6..];
                    }
                    {
                        let value = quoted(&mut rest)?;
                        let upto = offset + text.len() - rest.len();
                        accounting.add(
                            super::arxiv_accounting::Mode::Direct,
                            upto.checked_sub(*consumed).ok_or(Failure::Syntax)?,
                            true,
                        )?;
                        *consumed = upto;
                        if process {
                            super::arxiv_accounting::fragment(
                                &value,
                                &dtd.entities,
                                super::arxiv_accounting::Mode::None,
                                super::arxiv_accounting::Context::Attribute,
                                accounting,
                                deadline,
                            )
                            .await?;
                        }
                        if process {
                            let raw = expand(&value, &dtd.entities, text.len(), deadline)
                                .await?
                                .replace("\r\n", " ")
                                .replace(['\r', '\n', '\t'], " ");
                            if raw.contains('<') {
                                return Err(Failure::Syntax);
                            }
                            let decoded =
                                quick_xml::escape::unescape(&raw).map_err(|_| Failure::Syntax)?;
                            if !decoded.chars().all(xml_char) {
                                return Err(Failure::Syntax);
                            }
                            Some(
                                quick_xml::escape::escape(decoded.as_ref())
                                    .into_owned()
                                    .replace('\n', "&#10;")
                                    .replace('\r', "&#13;")
                                    .replace('\t', "&#9;"),
                            )
                        } else {
                            None
                        }
                    }
                };
                let attrs = dtd.attributes.entry(element.clone()).or_default();
                if process && !attrs.iter().any(|a| a.name == name) {
                    attrs.push(AttributeDecl {
                        name,
                        tokenized,
                        value,
                    });
                }
            }
        } else if declaration.starts_with("<!ELEMENT") || declaration.starts_with("<!NOTATION") {
            // No content-model validation is performed by source ElementTree.
            // Full declaration grammar validation remains separately audited.
        } else {
            return Err(Failure::Syntax);
        }
    }
    Ok(())
}
async fn expand(
    text: &str,
    entities: &BTreeMap<String, Option<String>>,
    _direct_bytes: usize,
    deadline: Option<Instant>,
) -> Result<String> {
    struct Frame<'a> {
        text: &'a str,
        pos: usize,
        out: String,
        tag: bool,
        quote: Option<char>,
        entity: Option<&'a str>,
    }
    let mut frames = vec![Frame {
        text,
        pos: 0,
        out: String::new(),
        tag: false,
        quote: None,
        entity: None,
    }];
    let mut indirect = 0usize;
    let mut resident = 0usize;
    let mut work = 0usize;
    loop {
        if work.is_multiple_of(4096) {
            checkpoint(deadline).await?;
        }
        work += 1;
        let Some(frame) = frames.last_mut() else {
            unreachable!()
        };
        if frame.pos == frame.text.len() {
            let mut done = frames.pop().unwrap().out;
            resident -= done.len();
            if let Some(parent) = frames.last_mut() {
                if let Some(q) = parent.quote {
                    if done.contains('<') {
                        return Err(Failure::Syntax);
                    }
                    done = done.replace(q, if q == '"' { "&quot;" } else { "&apos;" });
                }
                resident += done.len();
                if resident > MAX_TREE_BYTES {
                    return Err(Failure::Limit("entity allocation byte (134217728)"));
                }
                parent.out.push_str(&done);
                continue;
            }
            return Ok(done);
        }
        let tail = &frame.text[frame.pos..];
        if let Some((start, end)) = [("<!--", "-->"), ("<![CDATA[", "]]>"), ("<?", "?>")]
            .into_iter()
            .find(|(a, _)| tail.starts_with(a))
        {
            let len =
                tail[start.len()..].find(end).ok_or(Failure::Syntax)? + start.len() + end.len();
            frame.out.push_str(&tail[..len]);
            frame.pos += len;
            resident += len;
        } else {
            let c = tail.chars().next().unwrap();
            if c == '&' {
                let end = tail.find(';').ok_or(Failure::Syntax)?;
                let name = &tail[1..end];
                frame.pos += end + 1;
                // XML's predefined entities cannot be shadowed by declarations.
                if !matches!(name, "amp" | "lt" | "gt" | "apos" | "quot")
                    && let Some(value) = entities.get(name)
                {
                    let value = value.as_deref().ok_or(Failure::Syntax)?;
                    if frames.iter().any(|f| f.entity == Some(name)) {
                        return Err(Failure::Syntax);
                    }
                    indirect = indirect
                        .checked_add(value.len())
                        .ok_or(Failure::Limit("entity work byte"))?;
                    if indirect > MAX_INPUT_BYTES {
                        return Err(Failure::Limit("entity work byte (134217728)"));
                    }
                    if frames.len() * std::mem::size_of::<Frame<'_>>() > MAX_TREE_BYTES {
                        return Err(Failure::Limit("entity stack allocation byte"));
                    }
                    frames.push(Frame {
                        text: value,
                        pos: 0,
                        out: String::new(),
                        tag: false,
                        quote: None,
                        entity: Some(name),
                    });
                    continue;
                }
                frame.out.push_str(&tail[..=end]);
                resident += end + 1;
            } else {
                if frame.tag {
                    if let Some(q) = frame.quote {
                        if c == q {
                            frame.quote = None;
                        }
                    } else if c == '\'' || c == '"' {
                        frame.quote = Some(c);
                    } else if c == '>' {
                        frame.tag = false;
                    }
                } else if c == '<' {
                    frame.tag = true;
                }
                frame.out.push(c);
                frame.pos += c.len_utf8();
                resident += c.len_utf8();
            }
        }
        if resident > MAX_TREE_BYTES {
            return Err(Failure::Limit("expanded allocation byte (134217728)"));
        }
    }
}

async fn numeric_entity_value(value: &str, deadline: Option<Instant>) -> Result<String> {
    let normalized = value.replace("\r\n", "\n").replace('\r', "\n");
    let mut pos = 0;
    let mut out = String::new();
    let mut next = 0;
    while pos < normalized.len() {
        if pos >= next {
            checkpoint(deadline).await?;
            next = pos + 4096;
        }
        let tail = &normalized[pos..];
        if tail.starts_with("&#") {
            let end = tail.find(';').ok_or(Failure::Syntax)?;
            let r = quick_xml::events::BytesRef::new(&tail[1..end]);
            let c = r
                .resolve_char_ref()
                .map_err(|_| Failure::Syntax)?
                .ok_or(Failure::Syntax)?;
            if !xml_char(c) {
                return Err(Failure::Syntax);
            }
            out.push(c);
            pos += end + 1;
        } else {
            let c = tail.chars().next().unwrap();
            out.push(c);
            pos += c.len_utf8();
        }
    }
    Ok(out)
}

#[cfg(test)]
mod accounting_tests {
    use super::*;
    #[tokio::test]
    async fn pinned_expat_direct_and_indirect_trace_counts() {
        let (_, _, a) = prepare(
            "<!DOCTYPE r [<!ENTITY a 'x&amp;y'><!ENTITY b '&a;&a;'>]><r p='&b;'>&b;</r>",
            None,
        )
        .await
        .unwrap();
        assert_eq!((a.direct, a.indirect), (77, 44));
        let (_, _, a) = prepare(
            "<!DOCTYPE r [<!ENTITY a 'x&amp;y'><!ATTLIST r p CDATA '&a;'>]><r/>",
            None,
        )
        .await
        .unwrap();
        assert_eq!((a.direct, a.indirect), (66, 8));
    }
}
