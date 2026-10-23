"""Offline actual ElementTree/whole-arXiv XML grammar controls; temporary A06 oracle."""

import asyncio
import base64
import hashlib
import json
import pyexpat
import runpy
import xml.etree.ElementTree as ET
from pathlib import Path

BASE = runpy.run_path(str(Path(__file__).with_name("export_source.py")))
ATOM = "http://www.w3.org/2005/Atom"
BODY = f'<feed xmlns="{ATOM}"><entry><title>T</title><summary>S</summary></entry></feed>'


def cases():
    rows = []

    def add(name, xml):
        rows.append(
            {"id": "arxiv.grammar." + name, "engine": "arxiv", "query": "unused", "responses": [BASE["reply"](xml)]}
        )

    for i, model in enumerate(
        [
            "EMPTY",
            "ANY",
            "(entry)",
            "(entry,title+)",
            "((entry|title)*,summary?)",
            "(#PCDATA)",
            "(#PCDATA|entry)*",
            "(entry|title)",
            "(entry,title|summary)",
            "()",
            "(entry,)",
            "(entry|)",
            "(#PCDATA|entry)",
            "(#PCDATA,entry)*",
            "(entry)**",
            "ANY more",
            "(1entry)",
            "(entry title)",
            "(entry|title)+",
            "((entry))",
        ]
    ):
        add(f"element_{i}", f"<!DOCTYPE feed [<!ELEMENT feed {model}>]>" + BODY)
    for i, decl in enumerate(
        [
            "<!NOTATION n SYSTEM 'urn:x'>",
            "<!NOTATION n PUBLIC 'valid public'>",
            "<!NOTATION n PUBLIC 'valid' 'urn:x'>",
            "<!NOTATION n SYSTEM>",
            "<!NOTATION n PUBLIC 'bad&public'>",
            "<!NOTATION n 'x'>",
            "<!NOTATION n SYSTEM 'x' 'y'>",
            "<!NOTATION 1n SYSTEM 'x'>",
            "<!ENTITY a SYSTEM 'x' NDATA n>",
            "<!ENTITY a SYSTEM 'x' NDATA>",
            "<!ENTITY % a SYSTEM 'x' NDATA n>",
            "<!ENTITY a PUBLIC 'p' 'x'>",
            "<!ENTITY a PUBLIC 'p'>",
        ]
    ):
        add(f"notation_entity_{i}", f"<!DOCTYPE feed [{decl}]>" + BODY)
    for i, decl in enumerate(
        [
            "<!ATTLIST feed xmlns CDATA 'http://www.w3.org/2005/Atom'>",
            "<!ATTLIST feed xmlns CDATA #FIXED 'http://www.w3.org/2005/Atom'>",
            "<!ATTLIST feed xmlns CDATA #REQUIRED>",
            "<!ATTLIST feed xmlns CDATA #IMPLIED>",
            "<!ATTLIST feed x (a|b) 'a'>",
            "<!ATTLIST feed x () 'a'>",
            "<!ATTLIST feed x (a|) 'a'>",
            "<!ATTLIST feed x (1|two) '1'>",
            "<!ATTLIST feed x NOTATION (a|b) 'a'>",
            "<!ATTLIST feed x NOTATION (1|b) 'b'>",
            "<!ATTLIST feed x CDATA#IMPLIED>",
            "<!ATTLIST feed x CDATA #FIXED'x'>",
            "<!ATTLIST feed x WRONG 'x'>",
            "<!ATTLIST feed x CDATA '<bad'>",
            "<!ATTLIST feed x CDATA '&unknown;'>",
            "<!ATTLIST feed x CDATA '&#0;'>",
            "<!ATTLIST feed x CDATA #IMPLIED y CDATA 'ok'>",
        ]
    ):
        add(f"attribute_{i}", f"<!DOCTYPE feed [{decl}]>" + BODY.replace(f' xmlns="{ATOM}"', ""))
    for i, prefix in enumerate(
        [
            "<?xml version='1.0'?>",
            "<?xml version='1.1'?>",
            "<?xml version='2.0'?>",
            "<?xml version='1.'?>",
            "<?xml encoding='utf-8' version='1.0'?>",
            "<?xml version='1.0' standalone='yes' encoding='utf-8'?>",
            "<?xml version='1.0' encoding='utf-8' standalone='no'?>",
            "<?xml version='1.0' extra='x'?>",
            "<?xml version='1.0' standalone='bad'?>",
            " <?xml version='1.0'?>",
            "<!--x--><?xml version='1.0'?>",
            "<?XML version='1.0'?>",
            "<?xml-stylesheet href='x'?>",
            "<?go?>",
            "<?go x?>",
            "<?go?x?>",
            "<?1go x?>",
            "<?xml version='1.0'?><?xml version='1.0'?>",
        ]
    ):
        add(f"declaration_{i}", prefix + BODY)
    for i, decl in enumerate(
        [
            "<!ENTITY a 'A'><!ENTITY a 'B'>",
            "<!ENTITY a '&#38;b;'><!ENTITY b 'B'>",
            "<!ENTITY a '&amp;b;'><!ENTITY b 'B'>",
            "<!ENTITY a SYSTEM 'x'>",
            "<!ENTITY a SYSTEM 'x' NDATA n>",
            "<!ENTITY a '&a;'>",
            "<!ENTITY a '&b;'><!ENTITY b '&a;'>",
            "<!ENTITY % p \"<!ENTITY a 'A'>\">%p;",
            "<!ENTITY % p 'x'>%p;<!ENTITY a 'A'>",
            "<!ENTITY % p 'x'><!ENTITY a 'A'>",
            "%missing;<!ENTITY a 'A'>",
            "<!ENTITY % p 'x'><!ENTITY a '%p;'>",
            "<!ENTITY a '100%'>",
            "<!ENTITY a 'A'><!ATTLIST feed x CDATA '&a;'>",
        ]
    ):
        for standalone in ["no", "yes"]:
            add(
                f"parameter_{i}_{standalone}",
                f"<?xml version='1.0' standalone='{standalone}'?><!DOCTYPE feed [{decl}]>"
                + BODY.replace("<title>T", "<title>&a;"),
            )
    for i, doc in enumerate(
        [
            "feed SYSTEM 'x'",
            "feed PUBLIC 'p' 'x'",
            "feed PUBLIC 'p'",
            "feed GARBAGE",
            "feed[<!ELEMENT feed ANY>]",
            "feed [<!ELEMENT feed ANY>] more",
            "feed [<?xml version='1.0'?>]",
            "feed [<!--bad -- comment-->]",
            "feed [<![IGNORE[x]]>]",
            "feed [<!ENTITY a 'x'>]",
            "1feed",
            "feed []",
        ]
    ):
        add(f"doctype_{i}", f"<!DOCTYPE {doc}>" + BODY)
    for i, decl in enumerate(
        [
            "<!ATTLIST feed x CDATA '&missing;'>",
            "<!ATTLIST feed x CDATA '&later;'><!ENTITY later 'yes'>",
            "<!ENTITY earlier 'yes'><!ATTLIST feed x CDATA '&earlier;'>",
            "<!ENTITY ltchar '&#60;'><!ATTLIST feed x CDATA '&ltchar;'>",
            "<!ATTLIST feed x CDATA '&#60;'>",
            "<!ATTLIST feed x CDATA '&#0;'>",
            "<!ATTLIST unused x CDATA '&missing;'>",
            "%missing;<!ATTLIST feed x CDATA '&missing;'>",
            "<!ENTITY external SYSTEM 'x' NDATA n><!ATTLIST feed x CDATA '&external;'>",
        ]
    ):
        for standalone in ["no", "yes"]:
            add(
                f"default_semantics_{i}_{standalone}",
                f"<?xml version='1.0' standalone='{standalone}'?><!DOCTYPE feed [{decl}]>" + BODY,
            )
    for i, prefix in enumerate(
        [
            "<!DOCTYPE feed SYSTEM 'urn:[x]'>",
            "<!DOCTYPE feed PUBLIC 'p' 'urn:[x]'>",
            "<!DOCTYPE feed SYSTEM 'urn:[x]' [<!ENTITY a 'A'>]>",
        ]
    ):
        add(f"quoted_subset_{i}", prefix + BODY)
    for i, xml in enumerate(
        [
            BODY.replace("<title>T", "<title>]]>"),
            BODY + "\ufeff",
            "\ufeff" + BODY,
            "\ufeff\ufeff" + BODY,
            BODY.replace("<title>T", "<title>&#xFFFE;"),
            BODY.replace("<title>T", "<title>&#xFFFF;"),
            BODY.replace("<title>T", "<title>&#13;"),
            BODY.replace("<title>T", "<title>&#x0085;"),
            BODY.replace("<title>T", "<title a='x<y'>T"),
        ]
    ):
        add(f"xml_wellformed_{i}", xml)
    for level in [18, 19, 20]:
        declarations = "<!ENTITY e0 'x'>" + "".join(
            f"<!ENTITY e{j} '&e{j - 1};&e{j - 1};'>" for j in range(1, level + 1)
        )
        for context in ["attribute", "default", "content"]:
            for padding in ["none", "before", "after"]:
                filler = "<!--" + "p" * 100000 + "-->" if padding != "none" else ""
                decl = declarations + (f"<!ATTLIST entry pad CDATA '&e{level};'>" if context == "default" else "")
                body = BODY
                if context == "attribute":
                    body = body.replace("<entry>", f"<entry pad='&e{level};'>")
                if context == "content":
                    body = body.replace("<entry>", f"&e{level};<entry>")
                if padding == "before":
                    body = filler + body
                if padding == "after":
                    body = body + filler
                add(f"amplification_{level}_{context}_{padding}", f"<!DOCTYPE feed [{decl}]>" + body)
    return rows


async def main():
    rows = []
    for case in cases():
        record = await BASE["capture"](case)
        text = base64.b64decode(case["responses"][0]["body_b64"]).decode()
        try:
            ET.fromstring(text)
            record["elementtree"] = {"status": "accepted"}
        except ET.ParseError as e:
            record["elementtree"] = {"status": "ParseError", "message": str(e)}
        rows.append(record)
    paths = ["engines/arxiv.py", "slopsearx/adapter.py", "slopsearx/payload.py"]
    out = {
        "schema_version": 1,
        "plan_id": "A06",
        "baseline": "730c37a5f48b9830088594582cefb4c2f10a35e8",
        "status": "supplemental-xml-grammar-source-pack",
        "normalization": "latency_ms omitted; sets sorted; enum values serialized; no other output normalization",
        "runtime": {"expat": pyexpat.EXPAT_VERSION, "features": pyexpat.features},
        "source_sha256": {p: hashlib.sha256((BASE["ROOT"] / p).read_bytes()).hexdigest() for p in paths},
        "records": rows,
    }
    print(json.dumps(out, ensure_ascii=False, indent=2, allow_nan=False))


if __name__ == "__main__":
    asyncio.run(main())
