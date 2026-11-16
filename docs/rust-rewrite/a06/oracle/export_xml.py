"""Offline whole-adapter Atom parser contracts; temporary A06 migration oracle."""

import asyncio
import hashlib
import json
import runpy
from pathlib import Path

BASE = runpy.run_path(str(Path(__file__).with_name("export_source.py")))
ROOT = BASE["ROOT"]
ATOM = "http://www.w3.org/2005/Atom"
ARXIV = "http://arxiv.org/schemas/atom"


def cases():
    entry = "<entry><id>https://arxiv.org/abs/hep-th/9901001v3</id><title>T</title><summary>S</summary></entry>"
    feed = f'<feed xmlns="{ATOM}" xmlns:arxiv="{ARXIV}">{entry}</feed>'
    bodies = [
        ("empty", ""),
        ("malformed_close", feed[:-1]),
        ("two_roots", feed + feed),
        ("invalid_xml_character", feed.replace("<title>T", "<title>\x00")),
        ("wrong_root_name", feed.replace("feed", "unexpected")),
        ("entry_root", f'<entry xmlns="{ATOM}"><title>T</title></entry>'),
        ("no_namespace", feed.replace(f' xmlns="{ATOM}"', "")),
        ("prefix_namespace", f'<a:feed xmlns:a="{ATOM}"><a:entry><a:title>T</a:title></a:entry></a:feed>'),
        ("nested_entry", feed.replace(entry, "<wrapper>" + entry + "</wrapper>")),
        ("first_title_only", feed.replace("<title>T</title>", "<title>First</title><title>Second</title>")),
        ("mixed_title", feed.replace("<title>T</title>", "<title>Before<b>Child</b>After</title>")),
        ("leading_child_title", feed.replace("<title>T</title>", "<title><b>Child</b>After</title>")),
        ("comment_and_pi", feed.replace("<title>T</title>", "<title>A<!-- ignored -->B<?go now?>C</title>")),
        ("cdata_and_escaped", feed.replace("<title>T</title>", "<title><![CDATA[A<&]]>&amp;&#x1F419;</title>")),
        ("internal_entity", '<!DOCTYPE feed [<!ENTITY local "Local">]>' + feed.replace("<title>T", "<title>&local;")),
        (
            "external_entity",
            '<!DOCTYPE feed [<!ENTITY outside SYSTEM "file:///nonexistent-a06-canary">]>'
            + feed.replace("<title>T", "<title>&outside;"),
        ),
        ("undefined_entity", feed.replace("<title>T", "<title>&missing;")),
        (
            "unicode_encoding_declaration",
            '<?xml version="1.0" encoding="iso-8859-1"?>' + feed.replace("<title>T", "<title>café"),
        ),
        (
            "empty_metadata",
            feed.replace("</entry>", "<arxiv:doi/><arxiv:journal_ref/><arxiv:comment/><updated/></entry>"),
        ),
        (
            "populated_metadata",
            feed.replace(
                "</entry>",
                "<author><name> Ada </name></author><author><name> </name></author>"
                "<author><name>Bob</name></author><arxiv:doi> 10.1/test </arxiv:doi>"
                "<arxiv:journal_ref> Journal </arxiv:journal_ref><arxiv:comment> Note </arxiv:comment>"
                "<updated> 2024-01-02 </updated></entry>",
            ),
        ),
        (
            "nested_author_text",
            feed.replace("</entry>", "<author><name>Before<b>Child</b>After</name></author></entry>"),
        ),
        ("summary_unicode_whitespace", feed.replace("<summary>S", "<summary>A\u0085B\u00a0C\u2003D")),
        ("long_summary", feed.replace("<summary>S", "<summary>" + "🐙" * 301)),
        (
            "wrong_metadata_namespace",
            feed.replace("</entry>", '<doi>wrong</doi><author xmlns="urn:wrong"><name>Wrong</name></author></entry>'),
        ),
    ]
    return [
        {"id": "arxiv.xml." + name, "engine": "arxiv", "query": "q", "responses": [BASE["reply"](body)]}
        for name, body in bodies
    ]


async def main():
    paths = ["engines/arxiv.py", "slopsearx/adapter.py", "slopsearx/payload.py", "tests/test_payload.py"]
    out = {
        "schema_version": 1,
        "plan_id": "A06",
        "baseline": "730c37a5f48b9830088594582cefb4c2f10a35e8",
        "status": "supplemental-xml-source-pack",
        "normalization": "latency_ms omitted; sets sorted; enum values serialized; no other output normalization",
        "source_sha256": {p: hashlib.sha256((ROOT / p).read_bytes()).hexdigest() for p in paths},
        "records": [await BASE["capture"](case) for case in cases()],
    }
    print(json.dumps(out, ensure_ascii=False, indent=2, allow_nan=False))


if __name__ == "__main__":
    asyncio.run(main())
