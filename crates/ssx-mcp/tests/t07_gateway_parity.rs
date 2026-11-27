//! Direct-versus-native-gateway parity against the same real search surface.
#[path = "t06_support/fixture.rs"]
mod fixture;
#[path = "t06_support/server.rs"]
mod private_valkey;

use rmcp::{
    ServiceExt,
    model::{
        CallToolRequestParams, ClientConfig, GetPromptRequestParams, ProtocolVersion,
        ReadResourceRequestParams,
    },
    service::{RoleClient, RunningService},
    transport::{
        StreamableHttpClientTransport, streamable_http_client::StreamableHttpClientTransportConfig,
    },
};
use serde_json::{Value, json};
use ssx_mcp::{
    Result,
    gateway::Gateway,
    harness::{Host, http_router_for},
    search_surface::SearchSurface,
};
use std::time::Duration;
use tokio_util::sync::CancellationToken;

type Client = RunningService<RoleClient, ClientConfig>;

fn client_config() -> ClientConfig {
    ClientConfig::default().with_protocol_version(ProtocolVersion::V_2025_11_25)
}

async fn http_client(url: String) -> Result<Client> {
    let transport = StreamableHttpClientTransport::with_client(
        reqwest::Client::builder().no_proxy().build()?,
        StreamableHttpClientTransportConfig::with_uri(url),
    );
    Ok(client_config().serve(transport).await?)
}

#[tokio::test]
async fn direct_and_gateway_clients_preserve_search_surface_contracts() -> Result<()> {
    let valkey = private_valkey::Server::start();
    let fixture = fixture::Fixture::frozen(valkey.url(), false).await;
    let cancel = CancellationToken::new();
    let host = Host::start(
        http_router_for(
            fixture.surface.clone(),
            SearchSurface::catalogs(),
            cancel.clone(),
        ),
        cancel,
    )
    .await?;
    let url = format!("http://{}/mcp", host.address);

    let direct = http_client(url.clone()).await?;
    let gateway = Gateway::connect(url, None, Duration::from_secs(5)).await?;
    let handler = gateway.handler();
    let (server_io, client_io) = tokio::io::duplex(65536);
    let server = tokio::spawn(async move {
        handler.serve(server_io).await?.waiting().await?;
        Ok::<(), ssx_mcp::Error>(())
    });
    let forwarded = client_config().serve(client_io).await?;

    assert_eq!(
        serde_json::to_value(direct.list_tools(None).await?.tools)?,
        serde_json::to_value(forwarded.list_tools(None).await?.tools)?,
        "tool descriptors"
    );
    let direct_resources = direct.list_resources(None).await?.resources;
    let forwarded_resources = forwarded.list_resources(None).await?.resources;
    assert_eq!(
        serde_json::to_value(&direct_resources)?,
        serde_json::to_value(&forwarded_resources)?,
        "resource descriptors"
    );
    assert_eq!(
        serde_json::to_value(
            direct
                .list_resource_templates(None)
                .await?
                .resource_templates
        )?,
        serde_json::to_value(
            forwarded
                .list_resource_templates(None)
                .await?
                .resource_templates
        )?,
        "resource-template descriptors"
    );
    assert_eq!(
        serde_json::to_value(direct.list_prompts(None).await?.prompts)?,
        serde_json::to_value(forwarded.list_prompts(None).await?.prompts)?,
        "prompt descriptors"
    );

    for resource in direct_resources {
        let uri = resource.uri.clone();
        assert_eq!(
            serde_json::to_value(
                direct
                    .read_resource(ReadResourceRequestParams::new(uri.clone()))
                    .await?
            )?,
            serde_json::to_value(
                forwarded
                    .read_resource(ReadResourceRequestParams::new(uri.clone()))
                    .await?
            )?,
            "resource result for {uri}"
        );
    }

    let source: Value =
        serde_json::from_str(include_str!("../../../docs/rust-rewrite/t06/source.json"))?;
    for (name, call) in source["prompt_calls"]
        .as_object()
        .ok_or("missing prompt calls")?
    {
        let argument = call["argument"].as_str().ok_or("missing prompt argument")?;
        let mut arguments = serde_json::Map::new();
        arguments.insert(argument.to_owned(), json!("<CALLER_DATA>"));
        let params = GetPromptRequestParams::new(name.clone()).with_arguments(arguments);
        assert_eq!(
            serde_json::to_value(direct.get_prompt(params.clone()).await?)?,
            serde_json::to_value(forwarded.get_prompt(params).await?)?,
            "prompt result for {name}"
        );
    }

    let preview = CallToolRequestParams::new("slopsearx_explain_search_scope").with_arguments(
        serde_json::from_value(json!({"query":"gateway parity fixture"}))?,
    );
    assert_eq!(
        serde_json::to_value(direct.call_tool(preview.clone()).await?)?,
        serde_json::to_value(forwarded.call_tool(preview).await?)?,
        "successful search preview"
    );

    let invalid = CallToolRequestParams::new("slopsearx_explain_search_scope").with_arguments(
        serde_json::from_value(json!({"query":"gateway parity fixture","safesearch":"invalid"}))?,
    );
    assert_eq!(
        serde_json::to_value(direct.call_tool(invalid.clone()).await?)?,
        serde_json::to_value(forwarded.call_tool(invalid).await?)?,
        "structured tool error"
    );

    let unknown = CallToolRequestParams::new("not-a-real-search-tool");
    let direct_error = direct.call_tool(unknown.clone()).await.unwrap_err();
    let forwarded_error = forwarded.call_tool(unknown).await.unwrap_err();
    assert_eq!(
        format!("{direct_error:?}"),
        format!("{forwarded_error:?}"),
        "MCP error envelope"
    );

    direct.cancel().await?;
    forwarded.cancel().await?;
    server.await??;
    gateway.shutdown().await?;
    host.shutdown().await?;
    fixture.service.shutdown().await;
    Ok(())
}
