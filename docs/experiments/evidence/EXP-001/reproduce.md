# EXP-001 reproduction

Run from the registered checkout with the project development dependencies installed.
Extract the Python block below into `/private/tmp/exp001.py`, then run the command
in the registration. It binds only an ephemeral loopback port; engines and store
are fixtures. No production code changes or live engine requests.

```python
import asyncio, importlib.util, json, os, platform, subprocess
from pathlib import Path
from importlib.metadata import version

# Pin public MCP policy knobs; no operator credentials are copied into evidence.
for key in list(os.environ):
    if key.startswith('MCP_'):
        del os.environ[key]
from slopsearx.adapter import EngineStatus
from slopsearx.mcp.harness import FakeEngineSpec, make_fixture_http_app
spec = importlib.util.spec_from_file_location('transport_helpers', 'tests/test_mcp_harness.py')
h = importlib.util.module_from_spec(spec)
spec.loader.exec_module(h)
out = Path('docs/experiments/evidence/EXP-001')

def normalize(data):
    replacements = {}
    def collect(x):
        if isinstance(x, dict):
            for k,v in x.items():
                if k in ('query_id','cursor','result_id') and isinstance(v,str):
                    replacements[v] = '<'+k+'>'
                collect(v)
        elif isinstance(x,list):
            for v in x: collect(v)
    collect(data)
    def walk(x):
        if isinstance(x,dict):
            return {k: ('<elapsed>' if k == 'response_time_ms' else '<cache>' if k in ('cached','cached_error') else walk(v)) for k,v in x.items()}
        if isinstance(x,list): return [walk(v) for v in x]
        if isinstance(x,str):
            for a,b in sorted(replacements.items(),key=lambda kv:-len(kv[0])): x=x.replace(a,b)
        return x
    return walk(data)

def size(x): return len(json.dumps(x,sort_keys=True,separators=(',',':'),ensure_ascii=False).encode())

def facts(x):
    r=x['results'][0]
    return {'url':r.get('url'),'sources':r.get('source_engines'),
            'language':x['enforcement'].get('language'),
            'duckduckgo':[{k:o.get(k) for k in ('status','result_count')} for o in x['engine_outcomes'] if o['engine']=='duckduckgo']}

async def main():
    rows=[]
    scenarios=[('healthy',EngineStatus.OK,3),('rate_limited',EngineStatus.RATE_LIMITED,3),('timeout',EngineStatus.TIMEOUT,3),('empty',EngineStatus.OK,0)]
    for name,status,count in scenarios:
        for rep in (1,2):
            app=make_fixture_http_app([FakeEngineSpec('wikipedia',count=3),FakeEngineSpec('duckduckgo',count=count,status=status)])
            pair={}
            async with h._serve(app) as url:
                async with h._session(url) as (session,_):
                    await session.initialize()
                    for arm in (['baseline','candidate'] if rep==1 else ['candidate','baseline']):
                        args={'query':'experiment fixture','engines':['wikipedia','duckduckgo'],'language':'en','safesearch':'off','max_results':3,'freshness':'prefer_fresh'}
                        if arm=='candidate': args['include']=['results']
                        res=await session.call_tool('slopsearx_search',args)
                        data=h._payload(res)
                        (out/f'{name}-{rep}-{arm}.json').write_text(json.dumps(data,indent=2,sort_keys=True)+'\n')
                        if res.isError or 'error' in data: raise RuntimeError('tool error: '+str(data))
                        pair[arm]=data
            b,c=pair['baseline'],pair['candidate']
            bn,cn=normalize(b),normalize(c)
            bf,cf=facts(b),facts(c)
            row={'scenario':name,'repetition':rep,'baseline_bytes':size(bn),'candidate_bytes':size(cn),'baseline_raw_bytes':size(b),'candidate_raw_bytes':size(c),'baseline_facts':bf,'candidate_facts':cf,'results_equal':bn['results']==cn['results'],'scope_equal':bn['scope']==cn['scope'],'enforcement_equal':bn['enforcement']==cn['enforcement'],'tasks_equal':{k:bf[k]==cf[k] for k in bf},'tasks_available':{k:cf[k] is not None and cf[k]!=[] for k in cf}}
            rows.append(row)
    total_b=sum(r['baseline_bytes'] for r in rows); total_c=sum(r['candidate_bytes'] for r in rows)
    stable=all(all(rows[i][k]==rows[i+1][k] for k in ('baseline_bytes','candidate_bytes','baseline_facts','candidate_facts')) for i in range(0,len(rows),2))
    summary={'rows':rows,'baseline_total_bytes':total_b,'candidate_total_bytes':total_c,'reduction_fraction':1-total_c/total_b,'repeat_stable':stable,'all_guardrails_pass':all(r['results_equal'] and r['scope_equal'] and r['enforcement_equal'] and all(r['tasks_equal'].values()) and all(r['tasks_available'].values()) for r in rows),'environment':{'python':platform.python_version(),'system':platform.system(),'machine':platform.machine(),'registration_commit':subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),'packages':{p:version(p) for p in ['mcp','httpx','uvicorn','fastapi','pydantic']}}}
    (out/'summary.json').write_text(json.dumps(summary,indent=2,sort_keys=True)+'\n')
    print(json.dumps({k:v for k,v in summary.items() if k!='rows'},indent=2))
asyncio.run(main())
```

## Analyzer correction before full replay

Attempt 1 could not bind the loopback server in the sandbox. Attempt 2 reached
the server but the analyzer raised KeyError: the baseline enforcement object
was empty for this explicit-engine request. The final analyzer records missing
facts as null and fails task availability instead of crashing. No decision
threshold or candidate was changed. Attempt 2 responses and both stderr logs
are retained. The original analyzer follows for exact reproduction.

```python
import asyncio, importlib.util, json, os, platform, subprocess
from pathlib import Path
from importlib.metadata import version

# Pin public MCP policy knobs; no operator credentials are copied into evidence.
for key in list(os.environ):
    if key.startswith('MCP_'):
        del os.environ[key]
from slopsearx.adapter import EngineStatus
from slopsearx.mcp.harness import FakeEngineSpec, make_fixture_http_app
spec = importlib.util.spec_from_file_location('transport_helpers', 'tests/test_mcp_harness.py')
h = importlib.util.module_from_spec(spec)
spec.loader.exec_module(h)
out = Path('docs/experiments/evidence/EXP-001')

def normalize(data):
    replacements = {}
    def collect(x):
        if isinstance(x, dict):
            for k,v in x.items():
                if k in ('query_id','cursor','result_id') and isinstance(v,str):
                    replacements[v] = '<'+k+'>'
                collect(v)
        elif isinstance(x,list):
            for v in x: collect(v)
    collect(data)
    def walk(x):
        if isinstance(x,dict):
            return {k: ('<elapsed>' if k == 'response_time_ms' else '<cache>' if k in ('cached','cached_error') else walk(v)) for k,v in x.items()}
        if isinstance(x,list): return [walk(v) for v in x]
        if isinstance(x,str):
            for a,b in sorted(replacements.items(),key=lambda kv:-len(kv[0])): x=x.replace(a,b)
        return x
    return walk(data)

def size(x): return len(json.dumps(x,sort_keys=True,separators=(',',':'),ensure_ascii=False).encode())

def facts(x):
    r=x['results'][0]
    return {'url':r.get('url'),'sources':r.get('source_engines'),
            'language':x['enforcement']['language'],
            'duckduckgo':[{k:o.get(k) for k in ('status','result_count')} for o in x['engine_outcomes'] if o['engine']=='duckduckgo']}

async def main():
    rows=[]
    scenarios=[('healthy',EngineStatus.OK,3),('rate_limited',EngineStatus.RATE_LIMITED,3),('timeout',EngineStatus.TIMEOUT,3),('empty',EngineStatus.OK,0)]
    for name,status,count in scenarios:
        for rep in (1,2):
            app=make_fixture_http_app([FakeEngineSpec('wikipedia',count=3),FakeEngineSpec('duckduckgo',count=count,status=status)])
            pair={}
            async with h._serve(app) as url:
                async with h._session(url) as (session,_):
                    await session.initialize()
                    for arm in (['baseline','candidate'] if rep==1 else ['candidate','baseline']):
                        args={'query':'experiment fixture','engines':['wikipedia','duckduckgo'],'language':'en','safesearch':'off','max_results':3,'freshness':'prefer_fresh'}
                        if arm=='candidate': args['include']=['results']
                        res=await session.call_tool('slopsearx_search',args)
                        data=h._payload(res)
                        (out/f'{name}-{rep}-{arm}.json').write_text(json.dumps(data,indent=2,sort_keys=True)+'\n')
                        if res.isError or 'error' in data: raise RuntimeError('tool error: '+str(data))
                        pair[arm]=data
            b,c=pair['baseline'],pair['candidate']
            bn,cn=normalize(b),normalize(c)
            bf,cf=facts(b),facts(c)
            row={'scenario':name,'repetition':rep,'baseline_bytes':size(bn),'candidate_bytes':size(cn),'baseline_raw_bytes':size(b),'candidate_raw_bytes':size(c),'baseline_facts':bf,'candidate_facts':cf,'results_equal':bn['results']==cn['results'],'scope_equal':bn['scope']==cn['scope'],'enforcement_equal':bn['enforcement']==cn['enforcement'],'tasks_equal':{k:bf[k]==cf[k] for k in bf}}
            rows.append(row)
    total_b=sum(r['baseline_bytes'] for r in rows); total_c=sum(r['candidate_bytes'] for r in rows)
    stable=all(all(rows[i][k]==rows[i+1][k] for k in ('baseline_bytes','candidate_bytes','baseline_facts','candidate_facts')) for i in range(0,len(rows),2))
    summary={'rows':rows,'baseline_total_bytes':total_b,'candidate_total_bytes':total_c,'reduction_fraction':1-total_c/total_b,'repeat_stable':stable,'all_guardrails_pass':all(r['results_equal'] and r['scope_equal'] and r['enforcement_equal'] and all(r['tasks_equal'].values()) for r in rows),'environment':{'python':platform.python_version(),'system':platform.system(),'machine':platform.machine(),'registration_commit':subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),'packages':{p:version(p) for p in ['mcp','httpx','uvicorn','fastapi','pydantic']}}}
    (out/'summary.json').write_text(json.dumps(summary,indent=2,sort_keys=True)+'\n')
    print(json.dumps({k:v for k,v in summary.items() if k!='rows'},indent=2))
asyncio.run(main())
```
