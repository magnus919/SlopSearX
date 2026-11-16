"""Guarded temporary actual-wire snapshot/read oracle with fake engines only. G08."""
import asyncio
import importlib.util
import json
import sys
from pathlib import Path
spec=importlib.util.spec_from_file_location("t06_capture",Path(__file__).with_name("capture.py"));capture=importlib.util.module_from_spec(spec);sys.modules[spec.name]=capture;spec.loader.exec_module(capture)
async def main():
 from slopsearx.mcp.harness import build_fixture_context,create_fixture_server
 context=build_fixture_context()
 async def factory():return context
 rows=[]
 async with capture.WireClient(create_fixture_server(state_factory=factory)) as client:
  before={k:e.calls for k,e in context.active_engines.items()};first=await client.call_tool_mcp("slopsearx_search",{"query":"snapshot read","engines":["wikipedia"],"max_results":1,"include":[]});cursor=first['structuredContent']['meta']['cursor'];rows.append({"name":"slopsearx_search","arguments":{"query":"snapshot read","engines":["wikipedia"],"max_results":1,"include":[]},"result":first,"dispatch_delta":{k:e.calls-before[k] for k,e in context.active_engines.items()}})
  for name,args in [("read_results",{"cursor":"<CURSOR>","max_results":1}),("read_results",{"cursor":"<CURSOR>","max_results":1,"page":2}),("read_result",{"result_id":"<CURSOR>:0"}),("read_entities",{"cursor":"<CURSOR>","max_results":1}),("read_entities",{"cursor":"<CURSOR>","version":2}),("read_results",{"cursor":"<CURSOR>","page":99}),("read_result",{"result_id":"<CURSOR>:+1"}),("read_result",{"result_id":"<CURSOR>:١"}),("read_result",{"result_id":"<CURSOR>:1_0"}),("read_result",{"result_id":"<CURSOR>:"+"9"*200}),("read_result",{"result_id":"<CURSOR>:-1"}),("read_result",{"result_id":"<CURSOR>:1.0"}),("search",{"query":"snapshot read","engines":["wikipedia"],"max_results":2,"include":["results","payload"]}),("read_results",{"cursor":"<CURSOR>","max_results":50})]:
   before={k:e.calls for k,e in context.active_engines.items()};actual_args={k:v.replace('<CURSOR>',cursor) if isinstance(v,str) else v for k,v in args.items()};result=await client.call_tool_mcp("slopsearx_"+name,actual_args);rows.append({"name":"slopsearx_"+name,"arguments":args,"result":result,"dispatch_delta":{k:e.calls-before[k] for k,e in context.active_engines.items()}})
 Path(sys.argv[1]).write_text(json.dumps({"baseline":capture.BASELINE,"versions":capture.VERSIONS,"calls":rows},ensure_ascii=False,indent=2)+"\n")
if __name__=="__main__":asyncio.run(main())
