import asyncio
import hashlib
import json
import os
import pathlib
import subprocess
import sys
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

ROOT = pathlib.Path("/Users/magnus/.codex/worktrees/rust-a04-broad-api-adapters/SlopSearX")
sys.path.insert(0, str(ROOT))
if len(sys.argv) > 1:
    import importlib

    engine, label, base = sys.argv[1:]
    values = {"negative": -1, "negative_infinity": float("-inf"), "infinity": float("inf"), "huge": 1e300}

    async def run():
        mod = importlib.import_module("engines." + engine)
        cls = {
            "clinicaltrials": "ClinicalTrialsAdapter",
            "europepmc": "EuropePMCAdapter",
            "pubchem": "PubChemAdapter",
            "uniprot": "UniProtAdapter",
        }[engine]
        adapter = getattr(mod, cls)({"base_url": base, "timeout_ms": values[label]})
        try:
            r = await adapter.search("test")
            return {"status": r.status.value, "error_message": r.error_message, "result_count": len(r.results)}
        finally:
            await adapter.shutdown()

    print(json.dumps(asyncio.run(run())))
else:

    class Server(ThreadingHTTPServer):
        daemon_threads = False
        block_on_close = True

    rows = []
    for engine in ["clinicaltrials", "europepmc", "pubchem", "uniprot"]:
        for label in ["negative", "negative_infinity", "infinity", "huge"]:
            seen = []

            class Handler(BaseHTTPRequestHandler):
                def log_message(self, *args):
                    pass

                def do_GET(self):
                    row = {"path": self.path, "received_ns": time.monotonic_ns()}
                    seen.append(row)
                    try:
                        time.sleep(0.025)
                        data = json.dumps(
                            {
                                "clinicaltrials": {"studies": []},
                                "europepmc": {"resultList": {"result": []}},
                                "pubchem": {"IdentifierList": {"CID": []}},
                                "uniprot": {"results": []},
                            }[engine]
                        ).encode()
                        self.send_response(200)
                        self.send_header("Content-Length", str(len(data)))
                        self.end_headers()
                        self.wfile.write(data)
                    except (BrokenPipeError, ConnectionResetError):
                        pass
                    finally:
                        row["completed_ns"] = time.monotonic_ns()

            s = Server(("127.0.0.1", 0), Handler)
            t = threading.Thread(target=s.serve_forever, kwargs={"poll_interval": 0.005})
            t.start()
            p = subprocess.Popen(
                [sys.executable, "-O", __file__, engine, label, f"http://127.0.0.1:{s.server_port}"],
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                env={**os.environ, "NO_PROXY": "127.0.0.1", "HTTP_PROXY": "", "HTTPS_PROXY": "", "ALL_PROXY": ""},
            )
            killed = False
            try:
                out, err = p.communicate(timeout=3)
            except subprocess.TimeoutExpired:
                killed = True
                p.kill()
                out, err = p.communicate()
            s.shutdown()
            s.server_close()
            t.join()
            closed = time.monotonic_ns()
            if t.is_alive() or any(x.get("completed_ns", closed + 1) > closed for x in seen):
                raise RuntimeError("handler lifetime")
            rows.append(
                {
                    "engine": engine,
                    "timeout": label,
                    "requests": seen,
                    "exit": p.returncode,
                    "watchdog_killed_reaped": killed,
                    "stdout": out.decode(),
                    "stderr": err.decode(),
                    "closed_ns": closed,
                    "joined": True,
                }
            )
    sources = ["slopsearx/adapter.py"] + [
        "engines/" + e + ".py" for e in ["clinicaltrials", "europepmc", "pubchem", "uniprot"]
    ]
    pathlib.Path("/private/tmp/a07-timeout-loopback.json").write_text(
        json.dumps(
            {
                "python": sys.version,
                "sources": {p: hashlib.sha256((ROOT / p).read_bytes()).hexdigest() for p in sources},
                "cases": rows,
            },
            indent=2,
        )
        + "\n"
    )
