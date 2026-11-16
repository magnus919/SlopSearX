"""Guarded temporary pure source entity grammar oracle; no I/O beyond output. Retire G08."""
import copy
import dataclasses
import importlib.util
import json
import sys
import unicodedata
from pathlib import Path
spec=importlib.util.spec_from_file_location("t06_capture",Path(__file__).with_name("capture.py"));capture=importlib.util.module_from_spec(spec);sys.modules[spec.name]=capture;spec.loader.exec_module(capture)
from slopsearx.adapter import SearchResult
from slopsearx.mcp.entity_projection import entity_projection
from slopsearx.payload import build_payload
from slopsearx.service import ScopeDecision
from slopsearx.snapshot import SearchSnapshot

def row(domain="security",kind="vulnerability",data=None,engine="nvd"):
 return SearchResult(url="https://example.com/lead",title="same title",content="lead",engine=engine,engines={engine},payload=build_payload(domain,kind,{"cve_id":"CVE-2024-12345"} if data is None else data,engine=engine))
def package(name,engine="pypi",version=None,**facts):return row("packages","package",{"name":name,"version":version,**facts},engine)
cases=[]
def add(label,rows):
 snap=SearchSnapshot("snap-test","q","q1",rows,ScopeDecision(),len(rows),"default");records=[]
 for result in rows:
  value=dataclasses.asdict(result);value["engines"]=sorted(result.engines);records.append(value)
 for version in [1,2]:
  groups,relationships=entity_projection(snap,version);cases.append({"case":label,"version":version,"results":records,"entities":groups,"relationships":relationships})
add("conflicts_and_conservation",[row(data={"cve_id":"CVE-2024-12345","description":"one"}),package("requests",version="1"),SearchResult(url="https://unknown",title="same",content="lead",engine="wikipedia"),row(data={"cve_id":"cve-2024-12345","description":"two"})])
add("release_coordinates",[package("Some_Pkg",version="1"),package("some.pkg",version="1"),package("some-pkg",engine="npm",version="1"),package("some-pkg",version="2"),package("some-pkg"),package("@scope/pkg",engine="npm",version="1"),package("@Scope/pkg",engine="npm",version="1")])
for value in [None,1,[],{},"","CVE-2024-1","CVE-2024-12345 extra","x"*513]:add("invalid_cve_"+repr(value)[:20],[row(data={"cve_id":value}),row(data={"cve_id":value})])
for label,mutation in [("schema2",lambda p:p.update(schema_version=2)),("schema_bool",lambda p:p.update(schema_version=True)),("domain_jobs",lambda p:p.update(domain="jobs")),("provenance_none",lambda p:p.update(provenance=None)),("provenance_unknown",lambda p:p["provenance"].update(engine="unknown")),("unreported",lambda p:p["provenance"].update(adapter_fields=[])),("inferred",lambda p:p["provenance"].update(inferred_fields=["cve_id"])),("inferred_none",lambda p:p["provenance"].update(inferred_fields=None)),("data_array",lambda p:p.update(data=[]))]:
 r=row();mutation(r.payload);add(label,[r])
add("ambiguous_packages",[package("pkg",engine="npm",ecosystem="pypi"),package("pkg",engine="crates"),package("pkg",engine="npm",version=["1"]),package("@a/b/c",engine="npm")])
r=row();r.engine="cve";r.engines={"cve","nvd"};add("merged_provenance",[r])
add("large_conservation",[row() for _ in range(500)]+[row(data={"cve_id":None}) for _ in range(500)])
add("scholarly_aliases",[row("science","publication",{"doi":"https://doi.org/10.1000/CAFÉ","pmid":"12345678","pmcid":"pmc87654321","openalex_id":"https://openalex.org/w2626778328"},"source")])
for value in ["10.1000/CAFE\u0301","10.1000/CAFÉ","doi:10.1000/Example","https://doi.org/10.1000/a%CC%8A","10.1000/Å","10.1000/ﬃ","10.1000/Ａ","10.1000/А","10.1000/A"]:add("doi_unicode_"+value,[row("science","publication",{"doi":value},"source")])
add("repositories",[row("code","repository",{"repository":"Owner/Repo"},"github"),row("code","repository",{"repository":"https://github.com/owner/repo.git"},"github")])
add("repository_conflict",[row("code","repository",{"repository":"owner/one","repository_url":"https://github.com/owner/two"},"github")])
add("candidate_and_advisory",[package("pkg",version="1",repository_url="https://github.com/Org/Repo"),row(data={"cve_id":"CVE-2026-12345","ghsa_id":"GHSA-2345-cfgh-jmpq"})])
for data in [{"doi":"same title"},{"pmid":"0"},{"pmid":"1234567890"},{"pmcid":"PMC12 extra"},{"openalex_id":"https://evil.example/W2626778328"},{"repository":"https://github.com/owner/repo/issues"},{"ghsa_id":"GHSA-aaaa-bbbb-cccc"},{"doi":"10.1000/"+"x"*513}]:
 domain,kind=("code","repository") if "repository" in data else (("security","advisory") if "ghsa_id" in data else ("science","publication"));add("malformed_"+next(iter(data)),[row(domain,kind,data,"source")])
Path(sys.argv[1]).write_text(json.dumps({"baseline":capture.BASELINE,"unicode_version":unicodedata.unidata_version,"cases":cases},ensure_ascii=False,indent=2)+"\n")
