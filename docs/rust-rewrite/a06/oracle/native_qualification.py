"""Offline A06 native boundary qualification; no providers or Python runtime backend."""

import argparse
import json
import os
import socket
import subprocess
import tempfile
from pathlib import Path


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--build-messages", type=Path, required=True)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[4]
    out = None
    for line in args.build_messages.read_text().splitlines():
        row = json.loads(line)
        if row.get("reason") == "build-script-executed" and "ssx-source-xml" in row.get("package_id", ""):
            out = Path(row["out_dir"])
    if out is None:
        raise SystemExit(
            "missing ssx-source-xml build-script receipt; "
            "run cargo build --locked -p ssx-source-xml --message-format=json"
        )
    with tempfile.TemporaryDirectory(prefix="ssx-a06-native-") as temp:
        probe = Path(temp) / "resource-probe"
        subprocess.run(
            [
                "cc",
                "-std=c11",
                "-O2",
                "-Wall",
                "-Wextra",
                "-Werror",
                str(root / "vendor/expat/tests/resource_probe.c"),
                str(out / "libssx_expat_wrapper.a"),
                str(out / "native-build/libexpat.a"),
                "-o",
                str(probe),
            ],
            check=True,
        )
        with (Path(temp) / "sentinel").open("wb") as sentinel:
            a, b = socket.socketpair()
            try:
                result = subprocess.run(
                    [str(probe), str(sentinel.fileno()), str(a.fileno())],
                    pass_fds=(sentinel.fileno(), a.fileno()),
                    input=b"",
                    capture_output=True,
                    timeout=10,
                    check=True,
                )
                if result.stdout != b"stdio-ok\n":
                    raise SystemExit("stdio/descriptor audit response mismatch")
                a.close()
                b.settimeout(1)
                if b.recv(1) != b"":
                    raise SystemExit("inherited socket did not close")
            finally:
                a.close()
                b.close()
        memory = subprocess.run([str(probe), "--memory"], capture_output=True, timeout=30, check=True)
        if os.uname().sysname == "Linux" and not memory.stdout.startswith(b"GNU_AS=1073741824 "):
            raise SystemExit("GNU absolute address-space qualification missing")
        print(memory.stdout.decode().strip())
        print(
            json.dumps(
                {
                    "descriptor_audit": "passed",
                    "inherited_file": "closed",
                    "inherited_AF_UNIX_socket": "closed-peer-EOF",
                    "stdio": "preserved",
                    "NOFILE": 3,
                    "platform": os.uname().sysname,
                }
            )
        )


if __name__ == "__main__":
    main()
