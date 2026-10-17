//! W03 complete calls over the actual authenticated native HTTP transport.
#[path = "t06_support/server.rs"]
mod private_valkey;
mod w03_support;
use rmcp::{
    ServiceExt,
    model::*,
    transport::{
        StreamableHttpClientTransport, streamable_http_client::StreamableHttpClientTransportConfig,
    },
};
use serde_json::{Value, json};
use ssx_mcp::{
    Result,
    harness::{Host, http_router_for},
    oauth::NetworkAuth,
    research_planning::PlanningRegistry,
};
use std::time::Duration;
use tokio_util::sync::CancellationToken;
async fn call<S: rmcp::Service<rmcp::RoleClient>>(
    client: &rmcp::service::RunningService<rmcp::RoleClient, S>,
    name: &str,
    args: Value,
) -> Value {
    let result = client
        .call_tool(
            CallToolRequestParams::new(name.to_owned())
                .with_arguments(args.as_object().unwrap().clone()),
        )
        .await
        .unwrap();
    let value = serde_json::to_value(result).unwrap();
    assert_ne!(value["isError"], json!(true), "{value}");
    let body: Value = serde_json::from_str(value["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(
        value["structuredContent"], body,
        "complete structuredContent must match text"
    );
    body
}
async fn settled<S: rmcp::Service<rmcp::RoleClient>>(
    client: &rmcp::service::RunningService<rmcp::RoleClient, S>,
    mut job: Value,
) -> Value {
    tokio::time::timeout(Duration::from_secs(10), async {
        while [Some("queued"), Some("running")].contains(&job["state"].as_str()) {
            tokio::time::sleep(Duration::from_millis(10)).await;
            job = call(client, "slopsearx_get_job", json!({"job_id":job["job_id"]})).await;
        }
        job
    })
    .await
    .unwrap()
}
#[tokio::test]
async fn actual_authenticated_http_preview_execute_followup_replay_and_completion() -> Result<()> {
    let server = private_valkey::Server::start();
    let (registry, fixture) = w03_support::setup(server.url()).await;
    let cancel = CancellationToken::new();
    let auth = NetworkAuth::Static(ssx_domain::oauth::Secret::new("w03-token".into())?);
    let router = http_router_for(
        registry.clone(),
        PlanningRegistry::catalogs(),
        cancel.clone(),
    )
    .map(|r| auth.mount(r));
    let host = Host::start(router, cancel).await?;
    let url = format!("http://{}/mcp", host.address);
    let http = reqwest::Client::builder().no_proxy().build()?;
    assert_eq!(
        http.post(&url)
            .json(&json!({"jsonrpc":"2.0","id":1,"method":"tools/list"}))
            .send()
            .await?
            .status(),
        401
    );
    assert_eq!(fixture.calls(), 0);
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(reqwest::header::AUTHORIZATION, "Bearer w03-token".parse()?);
    let http = reqwest::Client::builder()
        .no_proxy()
        .default_headers(headers)
        .build()?;
    let transport = StreamableHttpClientTransport::with_client(
        http,
        StreamableHttpClientTransportConfig::with_uri(url),
    );
    let client = ().serve(transport).await?;
    // Business refusals remain successful MCP calls with complete matching text
    // and structuredContent; declared-type failures above/below stay framework errors.
    assert_eq!(
        call(&client, "slopsearx_start_research", json!({"question":" "})).await,
        json!({"error":{"code":"invalid_input","message":"question is required","field":"question"}})
    );
    assert_eq!(
        call(&client, "slopsearx_get_job", json!({"job_id":"missing"})).await,
        json!({"error":{"code":"invalid_job_id","message":"unknown job id","field":"job_id"}})
    );
    assert_eq!(
        call(
            &client,
            "slopsearx_start_research",
            json!({"question":"q","strategy":"invented"})
        )
        .await,
        json!({"error":{"code":"invalid_input","message":"unknown strategy 'invented'","field":"strategy","valid_alternatives":["triangulate","broad","fresh","counterevidence"]}})
    );
    assert_eq!(fixture.calls(), 0);
    let tools = client.list_all_tools().await?;
    let names: Vec<_> = tools.iter().map(|t| t.name.as_ref()).collect();
    for name in ssx_mcp::research_planning::TOOL_NAMES {
        assert!(names.contains(&name));
    }
    for name in [
        "max_queries",
        "max_attempts",
        "max_engine_attempts",
        "max_results",
    ] {
        let mut args =
            json!({"question":"invalid","initial_plan":[{"query":"q","engines":["wikipedia"]}]});
        args[name] = json!(true);
        let invalid = client
            .call_tool(
                CallToolRequestParams::new("slopsearx_start_research")
                    .with_arguments(args.as_object().unwrap().clone()),
            )
            .await?;
        assert!(invalid.is_error.unwrap_or(false));
    }
    assert_eq!(fixture.calls(), 0);
    // Frozen descriptors prohibit undeclared top-level keys. Exercise the
    // actual authenticated transport before any job/budget/provider work.
    let keys_before: u64 = redis::cmd("DBSIZE").query(&mut server.connection())?;
    let declared_calls = [
        (
            "slopsearx_plan_research",
            json!({"question":"q","subquestions":[]}),
        ),
        (
            "slopsearx_plan_query_variants",
            json!({"question":"q","variants":[]}),
        ),
        (
            "slopsearx_plan_research_followup",
            json!({"job_id":"missing","query":"q","parent_attempt_id":"missing","evidence_result_ids":[],"rationale":"q"}),
        ),
        ("slopsearx_start_research", json!({"question":"q"})),
        ("slopsearx_get_job", json!({"job_id":"missing"})),
        ("slopsearx_cancel_job", json!({"job_id":"missing"})),
        ("slopsearx_retry_research", json!({"job_id":"missing"})),
        (
            "slopsearx_extend_research",
            json!({"job_id":"missing","query":"q"}),
        ),
        (
            "slopsearx_update_research",
            json!({"job_id":"missing","subquestion_states":{}}),
        ),
    ];
    assert_eq!(
        declared_calls.len(),
        ssx_mcp::research_planning::TOOL_NAMES.len()
    );
    for (name, mut args) in declared_calls {
        args["unexpected"] = json!(true);
        let invalid = client
            .call_tool(
                CallToolRequestParams::new(name).with_arguments(args.as_object().unwrap().clone()),
            )
            .await?;
        assert!(
            invalid.is_error.unwrap_or(false),
            "undeclared tool argument must reject before execution: {name}"
        );
        let serialized = serde_json::to_value(invalid)?;
        assert_eq!(
            serialized["content"][0]["text"],
            "undeclared tool argument: unexpected"
        );
        assert_eq!(fixture.calls(), 0);
        let keys_after: u64 = redis::cmd("DBSIZE").query(&mut server.connection())?;
        assert_eq!(
            keys_after, keys_before,
            "undeclared call must not create durable work: {name}"
        );
    }
    let misspelled = client
        .call_tool(
            CallToolRequestParams::new("slopsearx_start_research").with_arguments(
                json!({"question":"q","max_query":1})
                    .as_object()
                    .unwrap()
                    .clone(),
            ),
        )
        .await?;
    assert!(
        misspelled.is_error.unwrap_or(false),
        "misspelled option must not use defaults"
    );
    assert_eq!(
        serde_json::to_value(misspelled)?["content"][0]["text"],
        "undeclared tool argument: max_query"
    );
    assert_eq!(fixture.calls(), 0);
    assert_eq!(
        redis::cmd("DBSIZE").query::<u64>(&mut server.connection())?,
        keys_before
    );
    println!(
        "W03_UNDECLARED_ARGUMENTS_COMPLETE tools=9 misspelled=1 actual_http=true provider_calls=0 durable_key_count_unchanged=true"
    );
    let preview=call(&client,"slopsearx_plan_research",json!({"question":"compare maintenance and alternatives","engines":["wikipedia"],"subquestions":[{"id":"a","question":"maintenance"},{"id":"b","question":"alternatives"}],"max_queries":4,"max_attempts":4,"max_engine_attempts":4})).await;
    assert_eq!(preview["dispatched"], json!(false));
    assert_eq!(fixture.calls(), 0);
    let job = call(
        &client,
        preview["execution"]["tool"].as_str().unwrap(),
        preview["execution"]["arguments"].clone(),
    )
    .await;
    let job = settled(&client, job).await;
    assert_eq!(fixture.calls(), 3);
    let original = job["queries"][1]["attempts"][0].clone();
    let captured = call(
        &client,
        "slopsearx_read_results",
        json!({"cursor":original["cursor"]}),
    )
    .await;
    let followup=call(&client,"slopsearx_plan_research_followup",json!({"job_id":job["job_id"],"query":"new maintenance lead","parent_attempt_id":original["attempt_id"],"evidence_result_ids":[original["admitted_result_ids"][0]],"subquestion_id":"a","rationale":"Investigate this reading lead","engines":["wikipedia"]})).await;
    assert_eq!(followup["dispatched"], json!(false));
    assert_eq!(fixture.calls(), 3);
    let mut args = followup["execution"]["arguments"].clone();
    args["continuation_key"] = json!("evidence-step");
    let extended = call(&client, "slopsearx_extend_research", args.clone()).await;
    assert!(extended.get("error").is_none(), "{extended}");
    assert_eq!(extended["queries"][1]["attempts"][0], original);
    assert_eq!(
        extended["queries"][3]["evidence_result_ids"],
        args["evidence_result_ids"]
    );
    assert_eq!(fixture.calls(), 4);
    let replay = call(&client, "slopsearx_extend_research", args.clone()).await;
    assert_eq!(replay["budgets"], extended["budgets"]);
    assert_eq!(replay["queries"], extended["queries"]);
    assert_eq!(fixture.calls(), 4);
    let mut conflict = args.clone();
    conflict["query"] = json!("different");
    assert_eq!(
        call(&client, "slopsearx_extend_research", conflict).await,
        json!({"error":{"code":"idempotency_conflict","message":"continuation_key was used for another request"}})
    );
    let request = json!({"job_id":job["job_id"],"subquestion_states":{"a":"resolved"},"complete":true,"rationale":"enough leads"});
    let done = call(&client, "slopsearx_update_research", request.clone()).await;
    assert_eq!(done["unresolved_subquestions"], json!(["b"]));
    assert_eq!(done["queries"], extended["queries"]);
    assert_eq!(
        call(&client, "slopsearx_update_research", request).await,
        done
    );
    assert_eq!(
        call(
            &client,
            "slopsearx_retry_research",
            json!({"job_id":job["job_id"]})
        )
        .await["state"],
        done["state"]
    );
    assert_eq!(
        call(
            &client,
            "slopsearx_read_results",
            json!({"cursor":original["cursor"]})
        )
        .await["results"],
        captured["results"]
    );
    let variant=call(&client,"slopsearx_plan_query_variants",json!({"question":"CVE-2026-12345","variants":["CVE-2026-12345 advisory"],"engines":["wikipedia"],"max_queries":2})).await;
    let variant_job = call(
        &client,
        "slopsearx_start_research",
        variant["execution"]["arguments"].clone(),
    )
    .await;
    assert_eq!(
        variant_job["queries"][1]["planning_method"],
        json!("terminology_expansion")
    );
    let variant_job = settled(&client, variant_job).await;
    assert_eq!(variant_job["stop_reason"], json!("query_budget_exhausted"));
    assert_eq!(fixture.calls(), 6);
    client.cancel().await?;
    registry.close().await;
    host.shutdown().await?;
    fixture.service.shutdown().await;
    Ok(())
}
#[tokio::test]
async fn budget_exhaustion_completion_and_exact_cap_replay_over_actual_http() -> Result<()> {
    let server = private_valkey::Server::start();
    let (registry, fixture) = w03_support::setup(server.url()).await;
    let cancel = CancellationToken::new();
    let host = Host::start(
        http_router_for(
            registry.clone(),
            PlanningRegistry::catalogs(),
            cancel.clone(),
        ),
        cancel,
    )
    .await?;
    let http = reqwest::Client::builder().no_proxy().build()?;
    let client = ()
        .serve(StreamableHttpClientTransport::with_client(
            http,
            StreamableHttpClientTransportConfig::with_uri(format!("http://{}/mcp", host.address)),
        ))
        .await?;
    for (max_queries, max_results, reason) in [
        (4, 1, "result_budget_exhausted"),
        (1, 20, "query_budget_exhausted"),
    ] {
        let job=call(&client,"slopsearx_start_research",json!({"question":"bounded","max_queries":max_queries,"max_results":max_results,"initial_plan":[{"query":"bounded","engines":["wikipedia"]}]})).await;
        let job = settled(&client, job).await;
        assert_eq!(job["stop_reason"], json!(reason));
        assert_eq!(job["caller_completed"], json!(false));
        let request = json!({"job_id":job["job_id"],"subquestion_states":{},"complete":true,"rationale":"bounded evidence is sufficient"});
        let done = call(&client, "slopsearx_update_research", request.clone()).await;
        assert_eq!(done["stop_reason"], json!(reason));
        assert_eq!(done["caller_completed"], json!(true));
        assert_eq!(
            call(&client, "slopsearx_update_research", request.clone()).await,
            done
        );
        let mut conflict = request;
        conflict["rationale"] = json!("different decision");
        assert_eq!(
            call(&client, "slopsearx_update_research", conflict).await["error"]["code"],
            json!("idempotency_conflict")
        );
    }
    client.cancel().await?;
    registry.close().await;
    host.shutdown().await?;
    fixture.service.shutdown().await;
    Ok(())
}
struct ProcessClient {
    child: tokio::process::Child,
    input: Option<tokio::process::ChildStdin>,
    output: tokio::io::BufReader<tokio::process::ChildStdout>,
    id: u64,
}
impl ProcessClient {
    async fn start(url: &str) -> Result<Self> {
        let mut child = tokio::process::Command::new(env!("CARGO_BIN_EXE_planning-qualification"))
            .args(["stdio", url])
            .env_clear()
            .env("PATH", "")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::inherit())
            .kill_on_drop(true)
            .spawn()?;
        let input = child.stdin.take();
        let output = tokio::io::BufReader::new(child.stdout.take().unwrap());
        let mut c = Self {
            child,
            input,
            output,
            id: 0,
        };
        c.rpc("initialize",json!({"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"w03-test","version":"1"}})).await?;
        use tokio::io::AsyncWriteExt;
        c.input
            .as_mut()
            .unwrap()
            .write_all(b"{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n")
            .await?;
        Ok(c)
    }
    async fn rpc(&mut self, method: &str, params: Value) -> Result<Value> {
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
        self.id += 1;
        let mut bytes = serde_json::to_vec(
            &json!({"jsonrpc":"2.0","id":self.id,"method":method,"params":params}),
        )?;
        bytes.push(b'\n');
        self.input.as_mut().unwrap().write_all(&bytes).await?;
        self.input.as_mut().unwrap().flush().await?;
        loop {
            let mut line = String::new();
            let n = tokio::time::timeout(Duration::from_secs(15), self.output.read_line(&mut line))
                .await??;
            assert!(n > 0, "native stdio unexpected EOF");
            let row: Value = serde_json::from_str(&line)?;
            if row["id"] == json!(self.id) {
                assert!(row.get("error").is_none(), "{row}");
                return Ok(row["result"].clone());
            }
        }
    }
    async fn call(&mut self, name: &str, args: Value) -> Result<Value> {
        let result = self
            .rpc("tools/call", json!({"name":name,"arguments":args}))
            .await?;
        assert_ne!(result["isError"], json!(true), "{result}");
        let body: Value = serde_json::from_str(result["content"][0]["text"].as_str().unwrap())?;
        assert_eq!(body, result["structuredContent"]);
        Ok(body)
    }
    async fn finish(mut self) -> Result<()> {
        drop(self.input.take());
        let result = tokio::time::timeout(Duration::from_secs(10), self.child.wait()).await??;
        assert!(result.success(), "native process failed {result}");
        Ok(())
    }
}
#[tokio::test]
async fn real_native_stdio_process_preserves_complete_descriptors_preview_and_evidence_replay()
-> Result<()> {
    let server = private_valkey::Server::start();
    let mut c = ProcessClient::start(server.url()).await?;
    assert_eq!(
        c.call("slopsearx_get_job", json!({"job_id":"missing"}))
            .await?,
        json!({"error":{"code":"invalid_job_id","message":"unknown job id","field":"job_id"}})
    );
    assert_eq!(
        c.call("slopsearx_start_research", json!({"question":" "}))
            .await?,
        json!({"error":{"code":"invalid_input","message":"question is required","field":"question"}})
    );
    let catalog = c.rpc("tools/list", json!({})).await?;
    assert_eq!(catalog["tools"], PlanningRegistry::descriptors()["tools"]);
    let preview=c.call("slopsearx_plan_research",json!({"question":"compare maintenance and alternatives","engines":["wikipedia"],"subquestions":[{"id":"a","question":"maintenance"}],"max_queries":3})).await?;
    assert_eq!(preview["dispatched"], json!(false));
    let mut job = c
        .call(
            "slopsearx_start_research",
            preview["execution"]["arguments"].clone(),
        )
        .await?;
    let until = tokio::time::Instant::now() + Duration::from_secs(10);
    while [Some("queued"), Some("running")].contains(&job["state"].as_str()) {
        assert!(tokio::time::Instant::now() < until);
        tokio::time::sleep(Duration::from_millis(10)).await;
        job = c
            .call("slopsearx_get_job", json!({"job_id":job["job_id"]}))
            .await?;
    }
    let original = job["queries"][0]["attempts"][0].clone();
    let followup=c.call("slopsearx_plan_research_followup",json!({"job_id":job["job_id"],"query":"new evidence","parent_attempt_id":original["attempt_id"],"evidence_result_ids":[original["admitted_result_ids"][0]],"rationale":"Inspect this lead","subquestion_id":"a","engines":["wikipedia"]})).await?;
    let mut args = followup["execution"]["arguments"].clone();
    args["continuation_key"] = json!("stdio-step");
    let extended = c.call("slopsearx_extend_research", args.clone()).await?;
    assert_eq!(extended["queries"][0]["attempts"][0], original);
    assert_eq!(extended["budgets"]["used"]["attempts"], json!(3));
    let replay = c.call("slopsearx_extend_research", args).await?;
    assert_eq!(replay["queries"], extended["queries"]);
    assert_eq!(replay["budgets"], extended["budgets"]);
    c.finish().await?;
    Ok(())
}
async fn legacy_event(
    stream: &mut reqwest::Response,
    pending: &mut Vec<u8>,
) -> Result<(String, String)> {
    loop {
        if let Some(i) = pending.windows(2).position(|s| s == b"\n\n") {
            let bytes: Vec<_> = pending.drain(..i + 2).collect();
            let text = String::from_utf8(bytes)?;
            let kind = text
                .lines()
                .find_map(|s| s.strip_prefix("event: "))
                .unwrap_or("message")
                .to_owned();
            let data = text
                .lines()
                .filter_map(|s| s.strip_prefix("data: "))
                .collect::<Vec<_>>()
                .join("\n");
            if !data.is_empty() {
                return Ok((kind, data));
            }
        }
        let bytes = tokio::time::timeout(Duration::from_secs(10), stream.chunk())
            .await??
            .ok_or("legacy EOF")?;
        pending.extend_from_slice(&bytes);
        assert!(pending.len() < 1024 * 1024);
    }
}
#[tokio::test]
async fn actual_authenticated_legacy_sse_preview_has_exact_output_and_zero_dispatch() -> Result<()>
{
    use std::sync::Arc;
    let server = private_valkey::Server::start();
    let (registry, fixture) = w03_support::setup(server.url()).await;
    let cancel = CancellationToken::new();
    let handler = registry.clone();
    let legacy = ssx_mcp::legacy::Legacy::with_factory(
        Arc::new(move |wire, owner| handler.clone().bind_connection(wire, owner)),
        PlanningRegistry::catalogs(),
        "/messages/",
        Duration::from_secs(60),
        cancel.clone(),
    )?;
    let auth = NetworkAuth::Static(ssx_domain::oauth::Secret::new("w03-token".into())?);
    let host = Host::start(auth.mount(legacy.router("/sse")?), cancel).await?;
    let base = format!("http://{}", host.address);
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(reqwest::header::AUTHORIZATION, "Bearer w03-token".parse()?);
    let http = reqwest::Client::builder()
        .no_proxy()
        .default_headers(headers)
        .build()?;
    let mut stream = http.get(format!("{base}/sse")).send().await?;
    let mut pending = Vec::new();
    let (kind, endpoint) = legacy_event(&mut stream, &mut pending).await?;
    assert_eq!(kind, "endpoint");
    let post = format!("{base}{endpoint}");
    let init = json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"w03-legacy","version":"1"}}});
    assert!(
        http.post(&post)
            .json(&init)
            .send()
            .await?
            .status()
            .is_success()
    );
    let _ = legacy_event(&mut stream, &mut pending).await?;
    http.post(&post)
        .json(&json!({"jsonrpc":"2.0","method":"notifications/initialized"}))
        .send()
        .await?;
    let source: Value = serde_json::from_str(include_str!(
        "../../../docs/rust-rewrite/w03/source-observation-001.json"
    ))?;
    let row = &source["observations"][1];
    http.post(&post).json(&json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"slopsearx_plan_query_variants","arguments":row["arguments"]}})).send().await?;
    let (_, text) = legacy_event(&mut stream, &mut pending).await?;
    let response: Value = serde_json::from_str(&text)?;
    assert_eq!(response["result"]["structuredContent"], row["output"]);
    assert_eq!(fixture.calls(), 0);
    drop(stream);
    registry.close().await;
    host.shutdown().await?;
    fixture.service.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn actual_http_default_strategies_deadline_and_idempotent_admission() -> Result<()> {
    let server = private_valkey::Server::start();
    let (registry, fixture) = w03_support::setup(server.url()).await;
    let cancel = CancellationToken::new();
    let auth = NetworkAuth::Static(ssx_domain::oauth::Secret::new("w03-token".into())?);
    let host = Host::start(
        http_router_for(
            registry.clone(),
            PlanningRegistry::catalogs(),
            cancel.clone(),
        )
        .map(|r| auth.mount(r)),
        cancel,
    )
    .await?;
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(reqwest::header::AUTHORIZATION, "Bearer w03-token".parse()?);
    let http = reqwest::Client::builder()
        .no_proxy()
        .default_headers(headers)
        .build()?;
    let client = ()
        .serve(StreamableHttpClientTransport::with_client(
            http,
            StreamableHttpClientTransportConfig::with_uri(format!("http://{}/mcp", host.address)),
        ))
        .await?;
    for (i, strategy) in ["triangulate", "broad", "fresh"].into_iter().enumerate() {
        let args = json!({"question":format!("default source templates {strategy}"),"strategy":strategy,"max_queries":1,"max_engines_per_query":1,"idempotency_key":format!("{strategy}-once"),"deadline":"2099-01-01T00:00:00+00:00"});
        let initial = call(&client, "slopsearx_start_research", args.clone()).await;
        assert_eq!(initial["strategy"], json!(strategy), "{initial:?}");
        assert_eq!(initial["queries"].as_array().unwrap().len(), 1);
        assert_eq!(initial["deadline"].as_str().unwrap().len(), 32);
        let done = settled(&client, initial).await;
        assert_eq!(fixture.calls(), i + 1);
        let mut replay = call(&client, "slopsearx_start_research", args).await;
        assert_eq!(
            replay["note"],
            json!("returned existing job for idempotency_key")
        );
        replay.as_object_mut().unwrap().remove("note");
        let mut expected = done.clone();
        expected.as_object_mut().unwrap().remove("note");
        expected.as_object_mut().unwrap().remove("created_at");
        assert_eq!(replay, expected);
        assert_eq!(fixture.calls(), i + 1);
        // Source returns retained ordinary idempotency status before validating
        // new budgets/deadline/plan. These new arguments cannot dispatch work.
        let replay_edited=call(&client,"slopsearx_start_research",json!({"question":"edited replay","strategy":strategy,"idempotency_key":format!("{strategy}-once"),"max_results":0,"deadline":"invalid","initial_plan":[{"query":"forged new query","engines":["wikipedia"],"requires_intent_grant":false}]})).await;
        assert_eq!(replay_edited["job_id"], done["job_id"]);
        assert_eq!(replay_edited["queries"], done["queries"]);
        assert_eq!(replay_edited["budgets"], done["budgets"]);
        assert_eq!(
            replay_edited["note"],
            json!("returned existing job for idempotency_key")
        );
        assert_eq!(fixture.calls(), i + 1);
    }
    // The accepted T06 fake intentionally declares only general categories.
    // Its reference-template scope begins with inactive arxiv, so the same
    // current policy must deny counterevidence atomically before dispatch.
    let reference = call(&client,"slopsearx_start_research",json!({"question":"reference template eligibility","strategy":"counterevidence","max_queries":1,"max_engines_per_query":1})).await;
    assert_eq!(reference["error"]["code"], json!("invalid_scope"));
    assert_eq!(
        reference["error"]["message"],
        json!("unknown or inactive engines: arxiv (inactive)")
    );
    let invalid = call(
        &client,
        "slopsearx_start_research",
        json!({"question":"bad date","deadline":"invalid"}),
    )
    .await;
    assert_eq!(invalid["error"]["code"], json!("invalid_input"));
    let source = call(
        &client,
        "slopsearx_start_research",
        json!({"question":"composition","source":{"kind":"snapshot","id":"forged"}}),
    )
    .await;
    assert_eq!(source["error"]["code"], json!("invalid_input"));
    assert!(source["error"]["message"].as_str().unwrap().contains("W09"));
    assert_eq!(fixture.calls(), 3);
    client.cancel().await?;
    registry.close().await;
    host.shutdown().await?;
    fixture.service.shutdown().await;
    Ok(())
}
