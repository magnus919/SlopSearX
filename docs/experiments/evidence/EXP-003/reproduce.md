# EXP-003 reproduction

Save these blocks as the named files, then run
`PYTHONPATH="$PWD" /Volumes/tank01/magnus/git/SlopSearX/.venv/bin/python /private/tmp/exp003.py`
from the baseline worktree. All servers bind ephemeral loopback ports; all
engines and stores are offline fixtures.

## exp003.py

```python
import json,os,subprocess,sys,platform,hashlib
from pathlib import Path
from importlib.metadata import version
root=Path('docs/experiments/evidence/EXP-003');rows=[]
def normalize(data):
 replacements={}
 def collect(x):
  if isinstance(x,dict):
   for k,v in x.items():
    if k in ('query_id','cursor','result_id') and isinstance(v,str):replacements[v]='<'+k+'>'
    collect(v)
  elif isinstance(x,list):
   for v in x:collect(v)
 collect(data)
 def walk(x):
  if isinstance(x,dict):return {k:('<elapsed>' if k=='response_time_ms' else walk(v)) for k,v in x.items()}
  if isinstance(x,list):return [walk(v) for v in x]
  if isinstance(x,str):
   for a,b in sorted(replacements.items(),key=lambda kv:-len(kv[0])):x=x.replace(a,b)
  return x
 return walk(data)
def size(x):return len(json.dumps(x,sort_keys=True,separators=(',',':'),ensure_ascii=False).encode())
def fact(x,lang):
 e=x['enforcement'].get('language',{})
 return e.get('requested')==lang and e.get('status')=='unsupported' and bool(e.get('reason')) and e.get('enforced_by')==[]
for lang in ['omitted','en','de']:
 for scope in ['auto','explicit']:
  for rep in [1,2]:
   dest=root/f'{lang}-{scope}-{rep}';dest.mkdir(exist_ok=True)
   with (dest/'stdout.txt').open('w') as o,(dest/'stderr.txt').open('w') as e:
    r=subprocess.run([sys.executable,'/private/tmp/exp003-worker.py',str(dest),lang,scope,str(rep)],stdout=o,stderr=e,timeout=30)
   (dest/'exit-code.txt').write_text(str(r.returncode)+'\n');assert r.returncode==0,dest
   b=normalize(json.loads((dest/'baseline.json').read_text()));c=normalize(json.loads((dest/'candidate.json').read_text()))
   effective='en' if lang=='omitted' else lang
   row={'language':lang,'scope':scope,'rep':rep,'baseline_complete':fact(b,effective),'candidate_complete':fact(c,effective),'baseline_bytes':size(b),'candidate_bytes':size(c)}
   for x in [b,c]:
    x['enforcement'].pop('language',None)
    x['warnings']=[w for w in x['warnings'] if w!=f"language '{effective}' is not consumed by any adapter"]
   row['guardrail_equal']=b==c;rows.append(row)
   (root/'rows.json').write_text(json.dumps(rows,indent=2)+'\n')
   assert row['guardrail_equal'],dest
   print(str(dest),flush=True)
summary={'pairs':len(rows),'baseline_completeness':sum(r['baseline_complete'] for r in rows)/len(rows),'candidate_completeness':sum(r['candidate_complete'] for r in rows)/len(rows),'size_increase':sum(r['candidate_bytes'] for r in rows)/sum(r['baseline_bytes'] for r in rows)-1,'all_guardrails_equal':all(r['guardrail_equal'] for r in rows),'environment':{'python':platform.python_version(),'system':platform.system(),'machine':platform.machine(),'packages':{k:version(k) for k in ['mcp','httpx','uvicorn','fastapi','pydantic']}}}
(root/'summary.json').write_text(json.dumps(summary,indent=2)+'\n');print(summary)
```

## exp003-worker.py

```python
import asyncio,importlib.util,inspect,json,os,sys
from pathlib import Path
for k in list(os.environ):
 if k.startswith('MCP_'): del os.environ[k]
from slopsearx.mcp import tools as t
from slopsearx.mcp.harness import FakeEngineSpec,make_fixture_http_app
s=importlib.util.spec_from_file_location('helpers','tests/test_mcp_harness.py');h=importlib.util.module_from_spec(s);s.loader.exec_module(h)
base=t._core_filter_enforcement
source=inspect.getsource(base);assert 'if language and language != "en":' in source
ns=dict(t.__dict__);exec(source.replace('if language and language != "en":','if language:'),ns);candidate=ns['_core_filter_enforcement']
out=Path(sys.argv[1]);language=sys.argv[2];scope=sys.argv[3];rep=int(sys.argv[4]);out.mkdir(parents=True,exist_ok=True)
async def main():
 app=make_fixture_http_app([FakeEngineSpec('wikipedia',count=3),FakeEngineSpec('duckduckgo',count=3)])
 async with h._serve(app) as url:
  async with h._session(url) as (session,_):
   await session.initialize()
   for arm in (['baseline','candidate'] if rep==1 else ['candidate','baseline']):
    t._core_filter_enforcement=base if arm=='baseline' else candidate
    args={'query':'fixture','max_results':3,'freshness':'prefer_fresh'}
    if language!='omitted': args['language']=language
    if scope=='explicit':args['engines']=['wikipedia','duckduckgo']
    r=await session.call_tool('slopsearx_search',args);data=h._payload(r)
    (out/f'{arm}.json').write_text(json.dumps(data,indent=2,sort_keys=True)+'\n')
    assert not r.isError and 'error' not in data,data
 t._core_filter_enforcement=base
asyncio.run(main())
```

