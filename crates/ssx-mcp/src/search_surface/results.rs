use super::artifact;
use ssx_domain::{ResultId, SearchResult, SearchSnapshot};
use ssx_json::{Value, json};

const NOTE: &str = "SlopSearX did not fetch or verify the linked page";
fn engines(r: &SearchResult) -> Vec<String> {
    if r.engines.is_empty() {
        vec![r.engine.clone()]
    } else {
        r.engines.iter().cloned().collect()
    }
}
pub(super) fn card(r: &SearchResult, id: Option<&str>, requested: bool) -> Value {
    let c = ssx_application::retrieval::classify_retrieval_url(&r.url);
    let typed = id.and_then(|v| {
        let (s, i) = v.rsplit_once(':')?;
        Some(ResultId::new(
            ssx_domain::SnapshotId::new(s).ok()?,
            i.parse().ok()?,
        ))
    });
    let dto = crate::projection::result_card(
        r,
        typed.as_ref(),
        ssx_wire::McpRetrievalDto {
            contract: "slopsearx.retrieval_handoff".into(),
            version: 1,
            eligible: c.status == "ok",
            url_status: c.status.into(),
            url_reason: c.reason,
            scheme: c.scheme,
        },
    );
    let mut out = ssx_json::to_value(&dto).expect("card serialization");
    if let Some(id) = id {
        out["result_id"] = json!(id);
        out["artifact"] = artifact("result", id);
    }
    let persisted = ssx_domain::storage::encode_result(r)
        .ok()
        .and_then(|v| v.get("payload").cloned())
        .filter(|v| v.is_object());
    if let Some(payload) = persisted.filter(|p| {
        requested
            || ssx_json::to_vec(p).is_ok_and(|v| v.len() <= ssx_wire::PUBLIC_PAYLOAD_INLINE_BYTES)
    }) {
        out["payload"] = payload;
    } else {
        out.as_object_mut().expect("card").shift_remove("payload");
    }
    out
}
pub(super) fn record(r: &SearchResult, s: &SearchSnapshot, id: &str) -> Value {
    let source = engines(r);
    let available = r.content.chars().count() > 300;
    let media = ssx_json::to_value(r.media.as_ref().map(|m| ssx_wire::MediaDto {
        media_type: m.media_type.clone(),
        url: ssx_wire::nonempty(m.url.as_ref()),
        thumbnail: ssx_wire::nonempty(m.thumbnail.as_ref()),
        source: ssx_wire::nonempty(m.source.as_ref()),
        width: m.width,
        height: m.height,
        duration: m.duration.as_ref().map(|v| v.as_number().clone()),
    }))
    .expect("media");
    let payload = ssx_domain::storage::encode_result(r)
        .ok()
        .and_then(|v| v.get("payload").cloned())
        .filter(valid_payload)
        .unwrap_or(Value::Null);
    let classified = ssx_application::retrieval::classify_retrieval_url(&r.url);
    let handoff = json!({"contract":"slopsearx.retrieval_handoff","version":1,"result_id":id,"url":classified.url,"url_status":classified.status,"url_reason":classified.reason,"scheme":classified.scheme,"eligible":classified.status=="ok","snippet_only":!available,"verified":false,"verification_note":NOTE,"provenance":{"snapshot_cursor":s.id().as_str(),"query_id":s.query_id().as_str(),"query":s.query(),"source_engines":source}});
    let mut out = json!({"result_id":id,"artifact":artifact("result",id),"title":r.title,"url":r.url,"content":r.content,"content_available":available,"thumbnail":r.thumbnail,"img_src":r.img_src,"media":media,"payload":payload,"source_engines":source,"source_count":source.len(),"primary_engine":r.engine,"category":r.category,"published_at":r.published_date,"tier":r.tier,"position":r.position,"score":r.score,"citation":{"label":r.title,"url":r.url},"provenance":{"query":s.query(),"query_id":s.query_id().as_str(),"rank_explanation":s.ranking_explanation(),"source_engines":source},"snapshot":{"cursor":s.id().as_str(),"query":s.query(),"query_id":s.query_id().as_str(),"total":s.total()},"note":NOTE,"retrieval":handoff});
    if !available {
        out["content_unavailable_note"] =
            json!("full content unavailable (adapter returned snippet only)");
    }
    for (key, value) in ssx_wire::paper_fields(r.work_group.as_ref()) {
        out[key.as_str()] = value;
    }
    out
}
fn valid_payload(v: &Value) -> bool {
    (v["schema_version"].as_i64().is_some()
        || v["schema_version"].as_u64().is_some()
        || v["schema_version"].as_bool().is_some())
        && v["domain"].as_str().is_some_and(|s| !s.is_empty())
        && v["type"].as_str().is_some_and(|s| !s.is_empty())
        && v["data"].is_object()
}
