# Isolated retry runner

Save the following blocks as `/private/tmp/retry-exp001.py` and
`/private/tmp/exp001.py` respectively; run the first with the project Python
and PYTHONPATH set to the worktree. Each pair has a 30-second process timeout.

```python
import os,sys,json,subprocess
from pathlib import Path
root=Path('docs/experiments/evidence/EXP-001/retry-4');root.mkdir(exist_ok=True)
original=Path('/private/tmp/exp001.py').read_text()
worker=original.replace("out = Path('docs/experiments/evidence/EXP-001')", "out = Path(os.environ['EXP_OUTPUT'])")
worker=worker.replace('for name,status,count in scenarios:',"for name,status,count in [x for x in scenarios if x[0]==os.environ['EXP_SCENARIO']]:")
worker=worker.replace('for rep in (1,2):',"for rep in (int(os.environ['EXP_REP']),):")
worker=worker[:worker.index('    total_b=')]+"    (out/'row.json').write_text(json.dumps(rows[0],indent=2)+'\\n')\nasyncio.run(main())\n"
Path('/private/tmp/exp001-worker.py').write_text(worker)
(root/'reproduce.md').write_text('# Isolated retry runner\n\nSave the following blocks as `/private/tmp/retry-exp001.py` and\n`/private/tmp/exp001.py` respectively; run the first with the project Python\nand PYTHONPATH set to the worktree. Each pair has a 30-second process timeout.\n\n```python\n'+Path(__file__).read_text()+'```\n\n```python\n'+original+'```\n')
rows=[];runs=[]
for scenario,rep,diag in [('rate_limited',1,True)]+[(s,r,False) for s in ['healthy','rate_limited','timeout','empty'] for r in (1,2)]:
 dest=root/('diagnostic' if diag else f'{scenario}-{rep}');dest.mkdir(exist_ok=True)
 env=dict(os.environ,EXP_SCENARIO=scenario,EXP_REP=str(rep),EXP_OUTPUT=str(dest))
 try:
  with (dest/'stdout.txt').open('w') as out,(dest/'stderr.txt').open('w') as err:
   result=subprocess.run([sys.executable,'/private/tmp/exp001-worker.py'],env=env,stdout=out,stderr=err,timeout=30)
  code=result.returncode
 except subprocess.TimeoutExpired: code=124
 runs.append({'scenario':scenario,'rep':rep,'diagnostic':diag,'exit_code':code})
 (root/'runs.json').write_text(json.dumps(runs,indent=2)+'\n')
 print(runs[-1],flush=True)
 if code: sys.exit(code)
 if not diag: rows.append(json.loads((dest/'row.json').read_text()))
b=sum(r['baseline_bytes'] for r in rows);c=sum(r['candidate_bytes'] for r in rows)
stable=all(all(rows[i][k]==rows[i+1][k] for k in ['baseline_bytes','candidate_bytes','baseline_facts','candidate_facts']) for i in range(0,8,2))
summary={'rows':rows,'baseline_total_bytes':b,'candidate_total_bytes':c,'reduction_fraction':1-c/b,'repeat_stable':stable,'all_guardrails_pass':all(r['results_equal'] and r['scope_equal'] and r['enforcement_equal'] and all(r['tasks_equal'].values()) and all(r['tasks_available'].values()) for r in rows)}
(root/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
print({k:v for k,v in summary.items() if k!='rows'})
```

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
