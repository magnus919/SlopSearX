"""Guarded temporary actual-wire prompt validation oracle; fake state only. G08."""
import asyncio
import importlib.util
import json
import sys
from pathlib import Path
spec=importlib.util.spec_from_file_location("t06_capture",Path(__file__).with_name("capture.py"));capture=importlib.util.module_from_spec(spec);sys.modules[spec.name]=capture;spec.loader.exec_module(capture)
async def main():
 from slopsearx.mcp.harness import create_fixture_server
 rows=[]
 async with capture.WireClient(create_fixture_server()) as client:
  prompts=await client.list_prompts()
  for prompt in prompts:
   name=prompt['name'];key=prompt['arguments'][0]['name']
   for args in [{},{key:'caller','unexpected':'literal'},{key:1},{key:None},{key:True},{key:{}},{key:[]},{key:'Unicode café and <CALLER_DATA>'}]:rows.append({'name':name,'arguments':args,'result':await client.rpc('prompts/get',{'name':name,'arguments':args})})
  rows.append({'name':'unknown','arguments':{},'result':await client.rpc('prompts/get',{'name':'unknown','arguments':{}})})
 Path(sys.argv[1]).write_text(json.dumps({'baseline':capture.BASELINE,'versions':capture.VERSIONS,'cases':rows},ensure_ascii=False,indent=2)+'\n')
if __name__=='__main__':asyncio.run(main())
