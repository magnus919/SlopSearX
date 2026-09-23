# EXP-024 reproduction — initial runner (attempt 1)

Extract the block to `/private/tmp/exp024.py` and run the registered command
from the baseline worktree with project dependencies. This uses only local
fixture adapters and the real shared service; no production files are edited.

```python
import asyncio,copy,inspect,json,random,time,platform,subprocess,statistics,textwrap
from dataclasses import asdict
from pathlib import Path
from importlib.metadata import version
from slopsearx import merger
from slopsearx.adapter import EngineAdapter,AdapterResponse,SearchResult,EngineStatus
from slopsearx.service import AppContext,SearchService,SearchRequest,search_response_to_payload
out=Path('docs/experiments/evidence/EXP-024')
baseline=merger.PresenceRanker.rank
source=textwrap.dedent(inspect.getsource(baseline))
needle='    seen: dict[str, SearchResult] = {}'
assert source.count(needle)==1
source=source.replace(needle,needle+'\n    normalized: dict[str, str] = {}')
needle='            norm_url = _normalise_url(result.url)'
assert source.count(needle)==1
source=source.replace(needle,'            if result.url not in normalized:\n                normalized[result.url] = _normalise_url(result.url)\n            norm_url = normalized[result.url]')
ns=dict(merger.__dict__);exec(source,ns);candidate=ns['rank']
class Fixture(EngineAdapter):
 def __init__(self,name,family,overlap):
  super().__init__();self.name=name;self.family=family;self.overlap=overlap
 async def search(self,query,params=None):
  results=[]
  for i in range(20):
   prefix='shared' if i<self.overlap else self.name
   url=f'https://example.org/{prefix}/{i}'
   if self.family=='tracking' or (self.family=='mixed' and i%2):url+=f'?utm_source=fixture&item={i}'
   results.append(SearchResult(url=url,title=f'Item {i}',content=f'Detail {i}',engine=self.name))
  return AdapterResponse(results=results,status=EngineStatus.OK,latency_ms=0)
def norm(r):
 p=search_response_to_payload(r);p.pop('query_id',None);p.pop('response_time_ms',None);return p
async def main():
 urls=['https://example.org','HTTPS://EXAMPLE.ORG/a','https://example.org/a#part','https://example.org/a?','https://example.org/a?utm_source=x&v=1','https://example.org/?a=1&a=2&b=','relative/path','http://[broken','https://example.org/?q=\ud800']
 feeds={n:[SearchResult(url=u,title=str(i),engine=n,tier=1 if n=='a' else 2) for i,u in enumerate(urls)] for n in ['a','b']}
 ranker=merger.PresenceRanker(per_engine_budget={'a':4,'b':6})
 b=baseline(ranker,copy.deepcopy(feeds),'fixture');c=candidate(ranker,copy.deepcopy(feeds),'fixture')
 def enc(rs):
  ds=[asdict(r) for r in rs]
  for d in ds:d['engines']=sorted(d['engines'])
  return ds
 (out/'edge-results.json').write_text(json.dumps({'input_urls':urls,'baseline':enc(b),'candidate':enc(c)},indent=2)+'\n')
 assert enc(b)==enc(c)
 rng=random.Random(20260923);rows=[];references={};calls=0
 try:
  for family in ['plain','mixed','tracking']:
   for overlap in [0,10,20]:
    cell=f'{family}-{overlap}'
    engines={n:Fixture(n,family,overlap) for n in ['engine_a','engine_b','engine_c']}
    service=SearchService(AppContext(active_engines=engines));req=SearchRequest(query='fixture',engines=list(engines),freshness='prefer_fresh')
    for arm,fn in [('baseline',baseline),('candidate',candidate)]:
     merger.PresenceRanker.rank=fn
     for _ in range(10):
      data=norm(await service.search(req))
      if cell not in references:references[cell]=data
      assert data==references[cell],(cell,arm,'warmup mismatch')
    for block in range(40):
     order=['baseline','candidate'];rng.shuffle(order);row={'cell':cell,'block':block,'order':order}
     for arm in order:
      merger.PresenceRanker.rank=baseline if arm=='baseline' else candidate
      elapsed=0
      for _ in range(20):
       start=time.perf_counter_ns();response=await service.search(req);elapsed+=time.perf_counter_ns()-start
       assert norm(response)==references[cell],(cell,arm,'response mismatch');calls+=1
      row[arm+'_ms']=elapsed/20/1e6
     rows.append(row)
    (out/'blocks.json').write_text(json.dumps(rows,indent=2)+'\n')
    print('Completed',cell,flush=True)
 finally:merger.PresenceRanker.rank=baseline
 groups=[[r for r in rows if r['cell']==cell] for cell in references]
 means=[{'cell':g[0]['cell'],'baseline_ms':statistics.mean(x['baseline_ms'] for x in g),'candidate_ms':statistics.mean(x['candidate_ms'] for x in g)} for g in groups]
 b=statistics.mean(x['baseline_ms'] for x in means);c=statistics.mean(x['candidate_ms'] for x in means)
 boot=random.Random(20260924);absdiff=[];relative=[]
 for _ in range(5000):
  sample=[x for g in groups for x in boot.choices(g,k=40)]
  bb=statistics.mean(x['baseline_ms'] for x in sample);cc=statistics.mean(x['candidate_ms'] for x in sample)
  absdiff.append(bb-cc);relative.append(1-cc/bb)
 absdiff.sort();relative.sort()
 summary={'measured_calls':calls,'blocks':len(rows),'means':means,'baseline_ms':b,'candidate_ms':c,'absolute_saving_ms':b-c,'relative_saving':1-c/b,'absolute_95_interval_ms':[absdiff[124],absdiff[4874]],'relative_95_interval':[relative[124],relative[4874]],'responses_equal':True,'cell_latency_guardrail':all(x['candidate_ms']<=1.05*x['baseline_ms'] for x in means),'environment':{'python':platform.python_version(),'system':platform.system(),'machine':platform.machine(),'registration_commit':subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),'packages':{p:version(p) for p in ['httpx','pydantic','fastapi']}}}
 (out/'summary.json').write_text(json.dumps(summary,indent=2)+'\n');print(json.dumps(summary,indent=2))
asyncio.run(main())
```

## Corrected runner (attempt 2)

Attempt 1 exited before measurement: the edge-case constructor omitted the
required content field. Attempt 2 supplies constant `content="fixture"`. No
candidate, timing workload, threshold or comparison rule changed. Save the
following block for the completed run; initial stderr is preserved separately.

```python
import asyncio,copy,inspect,json,random,time,platform,subprocess,statistics,textwrap
from dataclasses import asdict
from pathlib import Path
from importlib.metadata import version
from slopsearx import merger
from slopsearx.adapter import EngineAdapter,AdapterResponse,SearchResult,EngineStatus
from slopsearx.service import AppContext,SearchService,SearchRequest,search_response_to_payload
out=Path('docs/experiments/evidence/EXP-024')
baseline=merger.PresenceRanker.rank
source=textwrap.dedent(inspect.getsource(baseline))
needle='    seen: dict[str, SearchResult] = {}'
assert source.count(needle)==1
source=source.replace(needle,needle+'\n    normalized: dict[str, str] = {}')
needle='            norm_url = _normalise_url(result.url)'
assert source.count(needle)==1
source=source.replace(needle,'            if result.url not in normalized:\n                normalized[result.url] = _normalise_url(result.url)\n            norm_url = normalized[result.url]')
ns=dict(merger.__dict__);exec(source,ns);candidate=ns['rank']
class Fixture(EngineAdapter):
 def __init__(self,name,family,overlap):
  super().__init__();self.name=name;self.family=family;self.overlap=overlap
 async def search(self,query,params=None):
  results=[]
  for i in range(20):
   prefix='shared' if i<self.overlap else self.name
   url=f'https://example.org/{prefix}/{i}'
   if self.family=='tracking' or (self.family=='mixed' and i%2):url+=f'?utm_source=fixture&item={i}'
   results.append(SearchResult(url=url,title=f'Item {i}',content=f'Detail {i}',engine=self.name))
  return AdapterResponse(results=results,status=EngineStatus.OK,latency_ms=0)
def norm(r):
 p=search_response_to_payload(r);p.pop('query_id',None);p.pop('response_time_ms',None);return p
async def main():
 urls=['https://example.org','HTTPS://EXAMPLE.ORG/a','https://example.org/a#part','https://example.org/a?','https://example.org/a?utm_source=x&v=1','https://example.org/?a=1&a=2&b=','relative/path','http://[broken','https://example.org/?q=\ud800']
 feeds={n:[SearchResult(url=u,title=str(i),content="fixture",engine=n,tier=1 if n=='a' else 2) for i,u in enumerate(urls)] for n in ['a','b']}
 ranker=merger.PresenceRanker(per_engine_budget={'a':4,'b':6})
 b=baseline(ranker,copy.deepcopy(feeds),'fixture');c=candidate(ranker,copy.deepcopy(feeds),'fixture')
 def enc(rs):
  ds=[asdict(r) for r in rs]
  for d in ds:d['engines']=sorted(d['engines'])
  return ds
 (out/'edge-results.json').write_text(json.dumps({'input_urls':urls,'baseline':enc(b),'candidate':enc(c)},indent=2)+'\n')
 assert enc(b)==enc(c)
 rng=random.Random(20260923);rows=[];references={};calls=0
 try:
  for family in ['plain','mixed','tracking']:
   for overlap in [0,10,20]:
    cell=f'{family}-{overlap}'
    engines={n:Fixture(n,family,overlap) for n in ['engine_a','engine_b','engine_c']}
    service=SearchService(AppContext(active_engines=engines));req=SearchRequest(query='fixture',engines=list(engines),freshness='prefer_fresh')
    for arm,fn in [('baseline',baseline),('candidate',candidate)]:
     merger.PresenceRanker.rank=fn
     for _ in range(10):
      data=norm(await service.search(req))
      if cell not in references:references[cell]=data
      assert data==references[cell],(cell,arm,'warmup mismatch')
    for block in range(40):
     order=['baseline','candidate'];rng.shuffle(order);row={'cell':cell,'block':block,'order':order}
     for arm in order:
      merger.PresenceRanker.rank=baseline if arm=='baseline' else candidate
      elapsed=0
      for _ in range(20):
       start=time.perf_counter_ns();response=await service.search(req);elapsed+=time.perf_counter_ns()-start
       assert norm(response)==references[cell],(cell,arm,'response mismatch');calls+=1
      row[arm+'_ms']=elapsed/20/1e6
     rows.append(row)
    (out/'blocks.json').write_text(json.dumps(rows,indent=2)+'\n')
    print('Completed',cell,flush=True)
 finally:merger.PresenceRanker.rank=baseline
 groups=[[r for r in rows if r['cell']==cell] for cell in references]
 means=[{'cell':g[0]['cell'],'baseline_ms':statistics.mean(x['baseline_ms'] for x in g),'candidate_ms':statistics.mean(x['candidate_ms'] for x in g)} for g in groups]
 b=statistics.mean(x['baseline_ms'] for x in means);c=statistics.mean(x['candidate_ms'] for x in means)
 boot=random.Random(20260924);absdiff=[];relative=[]
 for _ in range(5000):
  sample=[x for g in groups for x in boot.choices(g,k=40)]
  bb=statistics.mean(x['baseline_ms'] for x in sample);cc=statistics.mean(x['candidate_ms'] for x in sample)
  absdiff.append(bb-cc);relative.append(1-cc/bb)
 absdiff.sort();relative.sort()
 summary={'measured_calls':calls,'blocks':len(rows),'means':means,'baseline_ms':b,'candidate_ms':c,'absolute_saving_ms':b-c,'relative_saving':1-c/b,'absolute_95_interval_ms':[absdiff[124],absdiff[4874]],'relative_95_interval':[relative[124],relative[4874]],'responses_equal':True,'cell_latency_guardrail':all(x['candidate_ms']<=1.05*x['baseline_ms'] for x in means),'environment':{'python':platform.python_version(),'system':platform.system(),'machine':platform.machine(),'registration_commit':subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),'packages':{p:version(p) for p in ['httpx','pydantic','fastapi']}}}
 (out/'summary.json').write_text(json.dumps(summary,indent=2)+'\n');print(json.dumps(summary,indent=2))
asyncio.run(main())
```
