"""Guarded temporary actual-wire argument consumer boundary oracle. G08."""
import asyncio
import importlib.util
import json
import sys
from pathlib import Path
sys.dont_write_bytecode=True
spec=importlib.util.spec_from_file_location("t06_capture",Path(__file__).with_name("capture.py"));capture=importlib.util.module_from_spec(spec);sys.modules[spec.name]=capture;spec.loader.exec_module(capture)
async def main():
 from slopsearx.mcp.harness import build_fixture_context,create_fixture_server
 context=build_fixture_context()
 async def factory():return context
 seeds={"slopsearx_search":{"query":"numeric boundary","engines":["wikipedia"]},"slopsearx_read_results":{"cursor":"missing"},"slopsearx_read_entities":{"cursor":"missing"}}
 fields={"slopsearx_search":["max_results","interactive_timeout_ms"],"slopsearx_read_results":["page","max_results"],"slopsearx_read_entities":["version"]}
 values=[("huge_positive",10**399),("huge_negative",-(10**399)),("huge_numeric_string","9"*400),("huge_negative_string","-"+"9"*400),("arabic_decimal","١"),("fullwidth_decimal","１"),("signed_underscore","+1_0"),("underscore_float","1_0.0"),("double_underscore","1__0"),("zero_underscore","0_0"),("fraction_zero_string","1.000"),("whitespace_sign","  +1  "),("long_string_4300","9"*4300),("long_string_4301","9"*4301)]
 rows=[]
 async with capture.WireClient(create_fixture_server(state_factory=factory)) as client:
  for name,keys in fields.items():
   for key in keys:
    for label,value in values:
     args={**seeds[name],key:value};before={k:e.calls for k,e in context.active_engines.items()};result=await client.call_tool_mcp(name,args);rows.append({'name':name,'case':key+'_'+label,'arguments':args,'result':result,'dispatch_delta':{k:e.calls-before[k] for k,e in context.active_engines.items()}})
  for n in [4300,4301]:
   args={'result_id':'missing:'+'9'*n};result=await client.call_tool_mcp('slopsearx_read_result',args);rows.append({'name':'slopsearx_read_result','case':'index_digits_'+str(n),'arguments':args,'result':result,'dispatch_delta':{k:0 for k in context.active_engines}})
 Path(sys.argv[1]).write_text(json.dumps({'baseline':capture.BASELINE,'versions':capture.VERSIONS,'cases':rows},ensure_ascii=False,indent=2)+'\n')
if __name__=='__main__':asyncio.run(main())
