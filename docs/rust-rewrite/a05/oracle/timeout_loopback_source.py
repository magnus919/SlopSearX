"""Temporary actual-HTTPX loopback timing evidence; no provider calls (G08).

Parent owns the HTTP server and watchdog, so a stuck child cannot hide requests.
Elapsed values and loopback ports are raw observational telemetry, not golden bytes.
"""

import argparse  # noqa: I001 - pinned oracle import behavior
import asyncio
import dataclasses
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

ROOT = Path(__file__).resolve().parents[4]
sys.path.insert(0, str(ROOT))


def child(config):
    import engines  # noqa: F401 - pinned oracle import behavior
    from slopsearx.adapter import discover_engines

    spec = importlib.util.spec_from_file_location("a05_export", Path(__file__).with_name("export.py"))
    export = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(export)

    async def run():
        adapter = discover_engines({config["engine"]: config["config"]})[config["engine"]]
        try:
            response = await adapter.search("loopback timeout control", {"categories": []})
            outcome = export.normalize(dataclasses.asdict(response))
            outcome["status"] = response.status.value
            return {"response": outcome}
        except Exception as error:
            return {"raised": {"class": type(error).__name__, "message": str(error)}}
        finally:
            await adapter.shutdown()

    print(json.dumps(asyncio.run(run()), allow_nan=False))


class OwnedHTTPServer(ThreadingHTTPServer):
    daemon_threads = False
    block_on_close = True


def main(output, stall_only=False):
    spec = importlib.util.spec_from_file_location("a05_export", Path(__file__).with_name("export.py"))
    export = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(export)
    rows = []
    for engine in ["duckduckgo", "google"]:
        for label, value in [
            ("positive", 1000),
            ("zero", 0),
            ("negative", -1),
            ("nan", float("nan")),
            ("infinity", float("inf")),
            ("negative_infinity", float("-inf")),
        ]:
            for scenario in (
                ["stalled_primary"] if stall_only else ["immediate", "slow_primary", "slow_bootstrap", "fallback"]
            ):
                requests = []

                class Handler(BaseHTTPRequestHandler):
                    def log_message(self, *_args):
                        pass

                    def handle_request(self):
                        try:
                            self.execute_request()
                        finally:
                            # Each request owns its completion record before server_close joins it.
                            if hasattr(self, "observation"):
                                self.observation["completed_monotonic_ns"] = time.monotonic_ns()

                    def execute_request(self):
                        length = int(self.headers.get("Content-Length", "0"))
                        raw = self.rfile.read(length)
                        row = {
                            "method": self.command,
                            "path": self.path,
                            "headers": list(self.headers.raw_items()),
                            "body_hex": raw.hex(),
                            "received_monotonic_ns": time.monotonic_ns(),
                        }
                        self.observation = row
                        requests.append(row)
                        if scenario == "stalled_primary" and self.path.split("?")[0] == "/primary":
                            time.sleep(5)
                        if (scenario == "slow_primary" and self.path.split("?")[0] == "/primary") or (
                            scenario == "slow_bootstrap" and self.path == "/bootstrap"
                        ):
                            time.sleep(0.08)
                        body = (
                            export.HOME
                            if self.path == "/bootstrap"
                            else export.PRIMARY
                            if engine == "duckduckgo"
                            else export.G["SAMPLE_HTML"]
                        )
                        if scenario == "fallback" and self.path.split("?")[0] == "/primary":
                            body = "<html><body>unusable primary</body></html>"
                        encoded = body.encode()
                        row["response"] = {
                            "status": 200,
                            "headers": [
                                ["Content-Type", "text/html; charset=utf-8"],
                                ["Content-Length", str(len(encoded))],
                            ],
                            "body_hex": encoded.hex(),
                        }
                        try:
                            self.send_response(200)
                            for key, val in row["response"]["headers"]:
                                self.send_header(key, val)
                            self.end_headers()
                            self.wfile.write(encoded)
                            row["write"] = "completed"
                        except (BrokenPipeError, ConnectionResetError) as error:
                            row["write"] = type(error).__name__

                    do_GET = handle_request  # noqa: N815 - BaseHTTPRequestHandler API spelling
                    do_POST = handle_request  # noqa: N815 - BaseHTTPRequestHandler API spelling

                server = OwnedHTTPServer(("127.0.0.1", 0), Handler)
                thread = threading.Thread(target=server.serve_forever, kwargs={"poll_interval": 0.01}, daemon=True)
                thread.start()
                base = f"http://127.0.0.1:{server.server_port}"
                config = {
                    "engine": engine,
                    "config": {
                        "enabled": True,
                        "timeout_ms": value,
                        "base_url": base + "/primary",
                        "bootstrap_url": base + "/bootstrap",
                        "fallback_base_url": base + "/fallback",
                    },
                }
                start = time.monotonic_ns()
                process = subprocess.Popen(
                    [sys.executable, "-O", __file__, "--child", json.dumps(config)],
                    stdout=subprocess.PIPE,
                    stderr=subprocess.PIPE,
                    env={
                        **os.environ,
                        "NO_PROXY": "127.0.0.1",
                        "no_proxy": "127.0.0.1",
                        "HTTP_PROXY": "",
                        "HTTPS_PROXY": "",
                        "ALL_PROXY": "",
                    },
                )
                watchdog = False
                try:
                    stdout, stderr = process.communicate(timeout=3)
                except subprocess.TimeoutExpired:
                    watchdog = True
                    process.kill()
                    stdout, stderr = process.communicate()
                elapsed = time.monotonic_ns() - start
                server.shutdown()
                server.server_close()
                thread.join()
                closed_at = time.monotonic_ns()
                if thread.is_alive() or any(
                    "completed_monotonic_ns" not in request or request["completed_monotonic_ns"] > closed_at
                    for request in requests
                ):
                    raise RuntimeError("owned loopback handler did not terminate before server close")
                rows.append(
                    {
                        "id": f"{engine}_{label}_{scenario}",
                        "timeout_repr": repr(value),
                        "config": {**config["config"], "timeout_ms": repr(value)},
                        "watchdog_seconds": 3,
                        "watchdog_killed_and_reaped": watchdog,
                        "exit_code": process.returncode,
                        "elapsed_ns": elapsed,
                        "requests": requests,
                        "server_closed_monotonic_ns": closed_at,
                        "serve_forever_joined": not thread.is_alive(),
                        "handler_policy": {
                            "daemon_threads": server.daemon_threads,
                            "block_on_close": server.block_on_close,
                        },
                        "stdout": stdout.decode(),
                        "stderr": stderr.decode(),
                        "outcome": None if not stdout else json.loads(stdout),
                    }
                )
    sources = ["slopsearx/adapter.py", "engines/duckduckgo.py", "engines/google.py"]
    output.write_text(
        json.dumps(
            {
                "status": "actual original adapter HTTPX sockets to private loopback; raw timing telemetry",
                "source_files": [
                    {"path": name, "sha256": hashlib.sha256((ROOT / name).read_bytes()).hexdigest()} for name in sources
                ],
                "python": sys.version,
                "case_count": len(rows),
                "cases": rows,
            },
            indent=2,
            ensure_ascii=False,
            allow_nan=False,
        )
        + "\n"
    )


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path)
    parser.add_argument("--child")
    parser.add_argument("--stall-only", action="store_true")
    args = parser.parse_args()
    if args.child:
        child(json.loads(args.child))
    else:
        main(args.output, args.stall_only)
