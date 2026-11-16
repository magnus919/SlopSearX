"""Temporary manual guarded raw-argument, resource, and specialist reference oracle. G08."""
import asyncio
import importlib.util
import json
import os
import sys
from pathlib import Path
spec = importlib.util.spec_from_file_location("t06_capture", Path(__file__).with_name("capture.py"))
capture = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = capture
spec.loader.exec_module(capture)

async def main():
    from slopsearx.mcp.harness import FakeEngineSpec, build_fixture_context, create_fixture_server
    rows = []
    context = build_fixture_context()
    async def factory():
        return context
    async with capture.WireClient(create_fixture_server(state_factory=factory)) as client:
        for raw_args in ['{"query":1e400}', '{"query":"q","unexpected":1e400}', '{"query":"q","max_results":1e400}', '{"cursor":"missing","version":1e400}', '{"query":'+"1"+"0"*400+'}', '{"query":"q","unexpected":'+"1"+"0"*400+'}']:
            name = "slopsearx_read_entities" if '"cursor"' in raw_args else "slopsearx_search"
            client.sequence += 1
            raw = '{"jsonrpc":"2.0","id":'+str(client.sequence)+',"method":"tools/call","params":{"name":'+json.dumps(name)+',"arguments":'+raw_args+'}}'
            before = {k:e.calls for k,e in context.active_engines.items()}
            response = await client.http.post('/mcp', content=raw, headers={"content-type":"application/json"})
            rows.append({"raw_request":raw,"status":response.status_code,"body":response.text,"dispatch_delta":{k:e.calls-before[k] for k,e in context.active_engines.items()}})
        resources = []
        for uri in ["slopsearx://capabilities", "slopsearx://routing-profiles", "slopsearx://health/summary", "slopsearx://capabilities/wikipedia", "slopsearx://capabilities/UNKNOWN", "slopsearx://missing"]:
            resources.append({"uri":uri,"result":await client.rpc("resources/read",{"uri":uri})})
    os.environ["MCP_GRANT_JOBS"] = "1"
    os.environ["MCP_GRANT_SECURITY"] = "1"
    os.environ["MCP_GRANT_SCIENCE"] = "1"
    context = build_fixture_context([FakeEngineSpec(name=name) for name in ["wikipedia","brave","duckduckgo","greenhouse","cve","openalex"]])
    async def specialist_factory():
        return context
    specialists=[]
    async with capture.WireClient(create_fixture_server(state_factory=specialist_factory)) as client:
        for name,args in [("jobs",{"company":"example","sources":["greenhouse"]}),("jobs",{"company":"example","sources":["greenhouse"],"location":"New York","employment_type":"full_time"}),("jobs",{"company":" ","sources":["greenhouse"]}),("security",{"query":"q","engines":["cve"]}),("security",{"query":"q","engines":["cve"],"evidence_types":["BAD"]}),("science",{"query":"q","engines":["openalex"]}),("science",{"query":"q","engines":["openalex"],"date_from":"2024-01-01","date_to":"2024-12-31"}),("science",{"query":"q","engines":["openalex"],"source_types":["BAD"]})]:
            before={k:e.calls for k,e in context.active_engines.items()};result=await client.call_tool_mcp("slopsearx_search_"+name,args);specialists.append({"name":"slopsearx_search_"+name,"arguments":args,"result":result,"dispatch_delta":{k:e.calls-before[k] for k,e in context.active_engines.items()}})
    Path(sys.argv[1]).write_text(json.dumps({"baseline":capture.BASELINE,"versions":capture.VERSIONS,"raw_arguments":rows,"resources":resources,"specialists":specialists},ensure_ascii=False,indent=2)+"\n")
if __name__=="__main__":asyncio.run(main())
