# EXP-002 reproduction

Extract this block to `/private/tmp/exp002.py` and run the registered command
from the baseline checkout with project dependencies. No upstream requests or
production code edits are made. Candidate injection lasts only this process.

```python
import asyncio,inspect,json,random,time,platform,subprocess,statistics,hashlib
from pathlib import Path
from importlib.metadata import version
from slopsearx import merger
from slopsearx.adapter import EngineAdapter,AdapterResponse,SearchResult,EngineStatus
from slopsearx.service import AppContext,SearchService,SearchRequest,search_response_to_payload
out=Path('docs/experiments/evidence/EXP-002')
baseline=merger._normalise_url
source=inspect.getsource(baseline)
needle='    # urlencode re-encodes'
assert needle in source
source=source.replace(needle,'    if not parsed.query:\n        return urllib.parse.urlunparse(parsed)\n'+needle,1)
ns={};exec(source,ns);candidate=ns['_normalise_url']
class Fixture(EngineAdapter):
 def __init__(self,name,family):
  super().__init__();self.name=name;self.family=family
 async def search(self,query,params=None):
  return AdapterResponse(results=[SearchResult(url=f'https://example.org/{i}'+(f'?utm_source=fixture&item={i}' if self.family=='tracking' or (self.family=='mixed' and i%2) else ''),title=f'Item {i}',content=f'Detail {i}',engine=self.name) for i in range(20)],status=EngineStatus.OK,latency_ms=0)
def norm(r):
 p=search_response_to_payload(r)
 # Serializer's dynamic fields are top level; preserve all other evidence.
 p.pop('query_id',None);p.pop('response_time_ms',None)
 return p
async def main():
 edges=['https://example.org','HTTPS://EXAMPLE.ORG/a','https://example.org/a#part','https://example.org/a?','https://example.org/a?utm_source=x&v=1','https://example.org/?a=1&a=2&b=','relative/path','http://[broken','https://example.org/?q=\ud800']
 checks=[{'input':u,'baseline':baseline(u),'candidate':candidate(u)} for u in edges]
 (out/'edge-cases.json').write_text(json.dumps(checks,indent=2)+'\n')
 assert all(x['baseline']==x['candidate'] for x in checks)
 rng=random.Random(20260919);rows=[];references={};calls=0
 try:
  for family in ['plain','mixed','tracking']:
   engines={n:Fixture(n,family) for n in ['engine_a','engine_b','engine_c']}
   service=SearchService(AppContext(active_engines=engines))
   req=SearchRequest(query='fixture',engines=list(engines),freshness='prefer_fresh')
   for arm,fn in [('baseline',baseline),('candidate',candidate)]:
    merger._normalise_url=fn
    for _ in range(10):
     data=norm(await service.search(req))
     if family not in references: references[family]=data
     assert data==references[family],(family,arm,'warmup mismatch')
   for block in range(40):
    order=['baseline','candidate'];rng.shuffle(order);row={'family':family,'block':block,'order':order}
    for arm in order:
     merger._normalise_url=baseline if arm=='baseline' else candidate
     elapsed=0
     for _ in range(20):
      start=time.perf_counter_ns();response=await service.search(req);elapsed+=time.perf_counter_ns()-start
      assert norm(response)==references[family],(family,arm,'response mismatch')
      calls+=1
     row[arm+'_ms']=elapsed/20/1e6
    rows.append(row)
   (out/'blocks.json').write_text(json.dumps(rows,indent=2)+'\n')
 finally: merger._normalise_url=baseline
 groups=[[r for r in rows if r['family']==f] for f in ['plain','mixed','tracking']]
 means=[{'family':g[0]['family'],'baseline_ms':statistics.mean(x['baseline_ms'] for x in g),'candidate_ms':statistics.mean(x['candidate_ms'] for x in g)} for g in groups]
 b=statistics.mean(x['baseline_ms'] for x in means);c=statistics.mean(x['candidate_ms'] for x in means)
 boot=random.Random(20260920);absdiff=[];relative=[]
 for _ in range(5000):
  sample=[x for g in groups for x in boot.choices(g,k=40)]
  bb=statistics.mean(x['baseline_ms'] for x in sample);cc=statistics.mean(x['candidate_ms'] for x in sample)
  absdiff.append(bb-cc);relative.append(1-cc/bb)
 absdiff.sort();relative.sort()
 summary={'measured_calls':calls,'blocks':len(rows),'means':means,'baseline_ms':b,'candidate_ms':c,'absolute_saving_ms':b-c,'relative_saving':1-c/b,'absolute_95_interval_ms':[absdiff[124],absdiff[4874]],'relative_95_interval':[relative[124],relative[4874]],'responses_equal':True,'family_latency_guardrail':all(x['candidate_ms']<=1.05*x['baseline_ms'] for x in means),'environment':{'python':platform.python_version(),'system':platform.system(),'machine':platform.machine(),'registration_commit':subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),'packages':{p:version(p) for p in ['httpx','pydantic','fastapi']}}}
 (out/'summary.json').write_text(json.dumps(summary,indent=2)+'\n');print(json.dumps(summary,indent=2))
asyncio.run(main())
```
