//! Native Expat helper candidate; adapter HTTP/factory activation remains pending.
use base64::Engine as _;
use ssx_domain::{AdapterResponse, EngineStatus};
use ssx_infra::adapters::arxiv_expat::AtomPool;
use ssx_json::{Value, json};
use std::{num::NonZeroUsize, path::PathBuf, time::Duration};
use tokio::time::Instant;
fn normalized(response: &AdapterResponse) -> Value {
    assert!(
        response.results.iter().all(|result| result.media.is_none()),
        "actual science result unexpectedly contains media"
    );
    json!({"results":response.results.iter().map(|r|json!({"url":r.url,"title":r.title,"content":r.content,"engine":r.engine,"engines":r.engines.iter().collect::<Vec<_>>(),"score":r.score,"position":r.position,"category":r.category,"published_date":r.published_date,"thumbnail":r.thumbnail,"img_src":r.img_src,"tier":r.tier,"media":null,"payload":r.payload,"work_group":r.work_group})).collect::<Vec<_>>(),"status":response.status,"error_message":response.error_message,"answers":response.answers,"corrections":response.corrections,"infoboxes":response.infoboxes,"synthetic":response.synthetic})
}

fn pool() -> AtomPool {
    AtomPool::new(
        PathBuf::from(env!("CARGO_BIN_EXE_ssx-atom-parser")),
        NonZeroUsize::new(1).unwrap(),
    )
    .unwrap()
}
#[tokio::test]
async fn native_helper_matches_frozen_parse_outputs_with_two_explicit_version_deltas() {
    let pool = pool();
    let mut counts = Vec::new();
    let mut deltas = Vec::new();
    for source in [
        include_str!("../../../docs/rust-rewrite/a06/source-xml.json"),
        include_str!("../../../docs/rust-rewrite/a06/source-initial.json"),
        include_str!("../../../docs/rust-rewrite/a06/source-regressions.json"),
        include_str!("../../../docs/rust-rewrite/a06/source-shared.json"),
        include_str!("../../../docs/rust-rewrite/a06/source-xml-grammar.json"),
    ] {
        let corpus: Value = ssx_json::from_str(source).unwrap();
        let mut count = 0;
        for record in corpus["records"].as_array().unwrap() {
            if record["engine"] != json!("arxiv") || record["output"]["status"] != json!("ok") {
                continue;
            }
            let reply = record["responses"].as_array().unwrap().last().unwrap();
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(reply["body_b64"].as_str().unwrap())
                .unwrap();
            let text = std::str::from_utf8(&bytes).unwrap();
            let out = pool
                .parse(text, Instant::now() + Duration::from_secs(10))
                .await;
            let id = record["id"].as_str().unwrap();
            if ["arxiv.grammar.declaration_2", "arxiv.grammar.declaration_3"].contains(&id) {
                assert!(
                    !record["output"]["results"].as_array().unwrap().is_empty(),
                    "retain actual old success"
                );
                assert_eq!(
                    normalized(&out),
                    normalized(&AdapterResponse::default()),
                    "approved Expat upgrade rejection {id}"
                );
                deltas.push(id.to_owned());
            } else {
                assert_eq!(normalized(&out), record["output"], "{id}");
            }
            count += 1;
        }
        counts.push(count);
    }
    assert_eq!(counts, vec![24, 16, 10, 1, 165]);
    assert_eq!(
        deltas,
        vec!["arxiv.grammar.declaration_2", "arxiv.grammar.declaration_3"]
    );
    pool.shutdown().await.unwrap();
}
#[tokio::test]
async fn native_stream_keeps_only_source_text_and_checks_later_xml() {
    let p = pool();
    let text = "<!DOCTYPE x [<!ATTLIST x xmlns CDATA 'http://www.w3.org/2005/Atom'>]><x><entry><title>A<!--ignored--><?go ignored?>B<child/>tail</title><title>second</title><author><name>X<b/>tail</name><name>Y</name></author><summary>  a  b </summary></entry></x>";
    let out = p
        .parse(text, Instant::now() + Duration::from_secs(10))
        .await;
    assert_eq!(out.status, EngineStatus::Ok);
    assert_eq!(out.results[0].title, "AB");
    assert_eq!(out.results[0].content, "a b");
    assert_eq!(
        out.results[0].payload.as_ref().unwrap()["data"]["authors"],
        json!(["X", "Y"])
    );
    assert!(
        p.parse(
            &format!("{text}<broken>"),
            Instant::now() + Duration::from_secs(10)
        )
        .await
        .results
        .is_empty()
    );
    p.shutdown().await.unwrap();
}
#[test]
fn allocation_refusals_are_distinct_and_late_syntax_outranks_retained_refusal() {
    use ssx_source_xml::{Error, parse};
    assert!(matches!(parse("<x/>", 1), Err(Error::NativeMemory)));
    let xml = format!(
        "<x xmlns='http://www.w3.org/2005/Atom'><entry><title>{}</title></entry></x>",
        "x".repeat(300000)
    );
    assert!(matches!(parse(&xml, 512 * 1024), Err(Error::OutputMemory)));
    assert!(matches!(
        parse(&format!("{xml}<bad>"), 512 * 1024),
        Err(Error::Syntax)
    ));
}

fn stress_input(kind: &str) -> String {
    let atom = "http://www.w3.org/2005/Atom";
    match kind {
        "deep" => format!("{}{}", "<x>".repeat(100000), "</x>".repeat(100000)),
        "siblings" => format!(
            "<x xmlns=\"{atom}\">{}<entry><title>T</title></entry></x>",
            "<i/>".repeat(1000001)
        ),
        "entity_chain" => {
            let mut decl = String::from("<!ENTITY e0 'X'>");
            for i in 1..=10000 {
                decl.push_str(&format!("<!ENTITY e{i} '&e{};'>", i - 1));
            }
            format!(
                "<!DOCTYPE x [{decl}]><x xmlns=\"{atom}\"><entry><title>&e10000;</title></entry></x>"
            )
        }
        "ascii" | "utf8_replacement" => {
            let prefix = format!("<x xmlns=\"{atom}\"><ignored>");
            let suffix = "</ignored><entry><title>T</title></entry></x>";
            let pad = 32 * 1024 * 1024 - prefix.len() - suffix.len();
            format!(
                "{prefix}{}{suffix}",
                if kind == "ascii" {
                    "p".repeat(pad)
                } else {
                    "\u{fffd}".repeat(pad)
                }
            )
        }
        _ => unreachable!(),
    }
}
#[tokio::test]
async fn source_valid_large_deep_and_decoding_expansion_stress() {
    let corpus: Value = ssx_json::from_str(include_str!(
        "../../../docs/rust-rewrite/a06/source-xml-stress.json"
    ))
    .unwrap();
    let p = pool();
    let mut count = 0;
    for r in corpus["records"].as_array().unwrap() {
        let Some(expected) = r.get("output") else {
            continue;
        };
        let xml = stress_input(r["generator"]["kind"].as_str().unwrap());
        let got = p
            .parse(&xml, Instant::now() + Duration::from_secs(30))
            .await;
        assert_eq!(normalized(&got), *expected, "{}", r["id"]);
        count += 1;
    }
    assert_eq!(count, 5);
    p.shutdown().await.unwrap();
}
fn retained_input(late: bool) -> String {
    let mut decl = format!("<!ENTITY e0 '{}'>", "A".repeat(1024));
    for i in 1..=16 {
        decl.push_str(&format!("<!ENTITY e{i} '&e{};&e{};'>", i - 1, i - 1));
    }
    format!(
        "<!DOCTYPE x [{decl}]><!--{}--><x xmlns=\"http://www.w3.org/2005/Atom\"><entry><title>&e16;</title></entry></x>{}",
        "p".repeat(1024 * 1024),
        if late { "<bad>" } else { "" }
    )
}
#[tokio::test]
async fn candidate_retained_refusal_is_explicit_and_late_syntax_still_wins() {
    let source: Value = ssx_json::from_str(include_str!(
        "../../../docs/rust-rewrite/a06/source-xml-stress.json"
    ))
    .unwrap();
    let rows = source["records"].as_array().unwrap();
    assert_eq!(
        rows[5]["source_output_summary"]["results_summary"][0]["title_codepoints"],
        json!(67108864)
    );
    assert!(
        rows[6]["source_output_summary"]["results_summary"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let p = pool();
    let accepted = p
        .parse(
            &retained_input(false),
            Instant::now() + Duration::from_secs(10),
        )
        .await;
    assert_eq!(accepted.status, EngineStatus::Error);
    assert_eq!(
        accepted.error_message.as_deref(),
        Some("arXiv native XML resource limit exceeded")
    );
    let malformed = p
        .parse(
            &retained_input(true),
            Instant::now() + Duration::from_secs(10),
        )
        .await;
    assert_eq!(
        normalized(&malformed),
        normalized(&AdapterResponse::default())
    );
    p.shutdown().await.unwrap();
}

#[tokio::test]
async fn source_valid_large_abstract_does_not_hit_global_json_interpolation_limit() {
    let p = pool();
    let summary = "A".repeat(5 * 1024 * 1024);
    let xml = format!(
        "<x xmlns=\"http://www.w3.org/2005/Atom\"><entry><title>T</title><summary>{summary}</summary></entry></x>"
    );
    let mut out = p
        .parse(&xml, Instant::now() + Duration::from_secs(10))
        .await;
    assert_eq!(out.status, EngineStatus::Ok);
    assert_eq!(
        out.results[0].payload.as_ref().unwrap()["data"]["abstract"].as_str(),
        Some(summary.as_str())
    );
    use sha2::{Digest, Sha256};
    let source: Value = ssx_json::from_str(include_str!(
        "../../../docs/rust-rewrite/a06/source-xml-stress.json"
    ))
    .unwrap();
    let record = source["records"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == json!("arxiv.stress.large_abstract5m"))
        .unwrap();
    let abstract_value = out.results[0]
        .payload
        .as_mut()
        .unwrap()
        .as_object_mut()
        .unwrap()
        .get_mut("data")
        .unwrap()
        .as_object_mut()
        .unwrap()
        .shift_remove("abstract")
        .unwrap();
    let abstract_text = abstract_value.as_str().unwrap();
    assert_eq!(
        abstract_text.chars().count(),
        record["abstract_codepoints"].as_u64().unwrap() as usize
    );
    let digest = Sha256::digest(abstract_text.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    assert_eq!(digest, record["abstract_sha256"].as_str().unwrap());
    assert_eq!(normalized(&out), record["output_without_abstract"]);
    p.shutdown().await.unwrap();
}

#[tokio::test]
async fn admission_precedes_copy_and_shutdown_drains_preflight() {
    use std::{
        future::{Future, poll_fn},
        task::Poll,
    };
    let p = pool();
    let text = "<x/>";
    let mut first = Box::pin(p.parse(text, Instant::now() + Duration::from_secs(10)));
    poll_fn(|cx| {
        assert!(first.as_mut().poll(cx).is_pending());
        Poll::Ready(())
    })
    .await;
    assert_eq!(p.available_operations(), 0);
    assert_eq!(
        p.available_slots(),
        1,
        "no worker starts during preflight yield"
    );
    let second = p
        .parse(text, Instant::now() + Duration::from_secs(10))
        .await;
    assert_eq!(second.status, EngineStatus::Error);
    assert_eq!(
        second.error_message.as_deref(),
        Some("arXiv native XML parser admission saturated")
    );
    let (closed, out) = tokio::join!(p.shutdown(), first);
    closed.unwrap();
    assert_eq!(out.status, EngineStatus::Error);
    assert_eq!(p.available_operations(), 1);
    assert!(matches!(
        AtomPool::new(PathBuf::from("unused"), NonZeroUsize::new(2).unwrap()),
        Err(ssx_infra::process_pool::ProcessError::InvalidCapacity)
    ));
}

#[cfg(unix)]
#[tokio::test(flavor = "current_thread")]
async fn actual_native_expansion_cancellation_holds_admission_until_reap() {
    use std::{io::Read as _, os::unix::fs::PermissionsExt};
    // Required Linux and Darwin hosts have bounded, nonblocking child pipes.
    // Linux >=4.9 caps new pipes at this actual sysctl; Apple's pipe size table
    // caps Darwin pipes at 64 KiB. PIPE_BUF is not a capacity bound.
    let input = retained_input(false);
    let pipe_ceiling = if cfg!(target_os = "linux") {
        std::fs::read_to_string("/proc/sys/fs/pipe-max-size")
            .unwrap()
            .trim()
            .parse::<usize>()
            .unwrap()
    } else if cfg!(target_os = "macos") {
        64 * 1024
    } else {
        panic!("native cancellation fixture needs a verified pipe ceiling on this Unix host");
    };
    assert!(pipe_ceiling > 0 && input.len() > pipe_ceiling);
    let id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let wrapper =
        std::env::temp_dir().join(format!("a06-native-cancel-{}-{id}", std::process::id()));
    let pidfile = wrapper.with_extension("pid");
    let ready = wrapper.with_extension("ready");
    let start = wrapper.with_extension("start");
    let observation = wrapper.with_extension("observation");
    std::fs::write(
        &wrapper,
        format!(
            "#!/bin/sh\nprintf '%s\\n' \"$$\" > '{}'\n: > '{}'\nwhile [ ! -f '{}' ]; do :; done\nexec '{}'\n",
            pidfile.display(), ready.display(), start.display(),
            env!("CARGO_BIN_EXE_ssx-atom-parser")
        ),
    )
    .unwrap();
    std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o700)).unwrap();
    let pool = AtomPool::new(wrapper.clone(), NonZeroUsize::new(1).unwrap()).unwrap();
    let worker = pool.clone();
    let mut caller = Some(Box::pin(
        worker.parse(&input, Instant::now() + Duration::from_secs(10)),
    ));
    // READY is created only after printf has closed the complete PID file.
    // The wrapper owns no extra process, never reads stdin and execs in place.
    let readiness = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if ready.is_file() {
                let raw = std::fs::read_to_string(&pidfile).map_err(|e| e.to_string())?;
                let pid = raw.trim().parse::<u32>().map_err(|e| e.to_string())?;
                if pid == 0 || raw != format!("{pid}\n") {
                    return Err("wrapper did not publish one complete positive PID".to_owned());
                }
                return Ok(pid);
            }
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    });
    let pid = tokio::select! {
        completed = caller.as_mut().unwrap() => Err(format!("native request completed before cancellation readiness: {completed:?}")),
        ready = readiness => ready.map_err(|e| e.to_string()).and_then(|pid| pid),
    };
    // No assertion can bypass cancellation, pool shutdown or fixture removal.
    let observed = (|| -> Result<u32, String> {
        let pid = pid?;
        if pool.available_operations() != 0 {
            return Err("native request was not held under admission".into());
        }
        std::fs::write(&start, []).map_err(|e| e.to_string())?;
        let end = std::time::Instant::now() + Duration::from_secs(2);
        loop {
            // Intentionally no await: on this current-thread runtime the pool's
            // nonblocking writer cannot deliver the remaining original hostile
            // input or EOF. The real native image therefore stays in read_exact;
            // this test still makes no claim that Expat parse entry occurred.
            let out = std::fs::File::create(&observation).map_err(|e| e.to_string())?;
            let mut probe = std::process::Command::new("/bin/ps")
                .args(["-p", &pid.to_string(), "-o", "comm="])
                .stdout(out)
                .stderr(std::process::Stdio::null())
                .spawn()
                .map_err(|e| e.to_string())?;
            let status = loop {
                match probe.try_wait() {
                    Ok(Some(status)) => break status,
                    Ok(None) if std::time::Instant::now() < end => {
                        std::thread::sleep(Duration::from_millis(1))
                    }
                    other => {
                        let _ = probe.kill();
                        let _ = probe.wait();
                        return Err(format!("bounded owned ps observation failed: {other:?}"));
                    }
                }
            };
            let file = std::fs::File::open(&observation).map_err(|e| e.to_string())?;
            let mut image = String::new();
            file.take(4097)
                .read_to_string(&mut image)
                .map_err(|e| e.to_string())?;
            if image.len() > 4096 {
                return Err("ps observation exceeded bound".into());
            }
            if status.success() && image.trim().rsplit('/').next() == Some("ssx-atom-parser") {
                if pool.available_operations() != 0 {
                    return Err("native observation lost active admission".into());
                }
                return Ok(pid);
            }
            if std::time::Instant::now() >= end {
                return Err("real native image was not observed before the original bound".into());
            }
        }
    })();
    // Drop the pending caller BEFORE the first await following START. This
    // synchronously closes the reply receiver, without an abort-task scheduling
    // race. A completed call was rejected above; shutdown proves child reap.
    drop(caller.take());
    let admission_after_cancel = pool.available_operations();
    let closed = pool.shutdown().await;
    let available = pool.available_operations();
    let reaped = observed.as_ref().ok().map(|pid| {
        std::process::Command::new("/bin/kill")
            .args(["-0", &pid.to_string()])
            .stderr(std::process::Stdio::null())
            .status()
            .map(|status| !status.success())
    });
    for path in [&wrapper, &pidfile, &ready, &start, &observation] {
        let _ = std::fs::remove_file(path);
    }
    observed.unwrap();
    assert_eq!(admission_after_cancel, 0);
    closed.unwrap();
    assert_eq!(available, 1);
    assert!(
        matches!(reaped, Some(Ok(true))),
        "native PID reap observation: {reaped:?}"
    );
}
