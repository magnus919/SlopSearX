"""Offline migration oracle. Never invoked by native runtime/gates; retire at G08.
Run with pinned Python3.14.7 from an isolated accepted-source checkout.
"""

from __future__ import annotations

import ast
import asyncio
import hashlib
import json
import os
import platform
import subprocess
import sys
import tempfile
from pathlib import Path
from unittest.mock import patch

BASELINE = "1f3f5bc85083f4fd24bd9a2b07f161626e6bfef5"
SOURCES = [
    "slopsearx/portal_oidc.py",
    "slopsearx/portal_auth.py",
    "slopsearx/portal_rate_limit.py",
    "slopsearx/portal_audit.py",
    "slopsearx/workflow_portal.py",
    "tests/test_portal_oidc.py",
    "tests/test_portal_auth.py",
    "tests/test_portal_rate_limit.py",
    "tests/test_portal_audit.py",
    "tests/test_workflow_portal.py",
    "tests/test_workflow_portal_config.py",
]


def main() -> None:
    root = Path(sys.argv[1]).resolve()
    sys.path.insert(0, str(root))
    from slopsearx.mcp.harness import InMemoryStore
    from slopsearx.portal_auth import (
        PortalMembership,
        PortalPrincipal,
        PortalSessionStore,
        StaticMembershipResolver,
        safe_return_to,
    )
    from slopsearx.portal_oidc import OIDCSettings, _validate_claims, _verify_rs256
    from slopsearx.workflow_portal import _load_directory, _previous_secret

    if platform.python_version() != "3.14.7":
        raise RuntimeError("pinned Python3.14.7 required")
    sources = []
    for name in SOURCES:
        actual = (root / name).read_bytes()
        pinned = subprocess.check_output(["git", "show", f"{BASELINE}:{name}"], cwd=root)
        if actual != pinned:
            raise RuntimeError(f"source drift: {name}")
        sources.append({"path": name, "sha256": hashlib.sha256(actual).hexdigest()})
    settings = OIDCSettings(
        "https://id.example", "client", "synthetic-client-secret", "https://portal.example/auth/callback"
    )
    base = {"iss": "https://id.example", "sub": "subject", "aud": "client", "nonce": "nonce", "iat": 1000, "exp": 2000}
    variants = [
        ("valid", {}, 1100),
        ("issuer_trailing_slash", {"iss": "https://id.example/"}, 1100),
        ("wrong_issuer", {"iss": "https://attacker"}, 1100),
        ("expired", {"exp": 1000}, 1100),
        ("skew_inside", {}, 2029.999),
        ("skew_boundary", {}, 2030),
        ("iat_future", {"iat": 1131}, 1100),
        ("nbf_future", {"nbf": 1131}, 1100),
        ("wrong_nonce", {"nonce": "wrong"}, 1100),
        ("empty_subject", {"sub": ""}, 1100),
        ("wrong_audience", {"aud": "wrong"}, 1100),
        ("multiple_azp", {"aud": ["client", "other"], "azp": "client"}, 1100),
        ("multiple_missing_azp", {"aud": ["client", "other"]}, 1100),
        ("single_wrong_azp", {"azp": "wrong"}, 1100),
        ("mixed_audience", {"aud": ["client", False], "azp": "client"}, 1100),
        ("nan_exp", {"exp": "NaN"}, 1100),
        ("nan_iat", {"iat": "NaN"}, 1100),
        ("nan_nbf", {"nbf": "NaN"}, 1100),
        ("infinite_exp", {"exp": "Infinity"}, 1100),
        ("bool_iat", {"iat": True}, 1100),
        ("numeric_string_iat", {"iat": "1000"}, 1100),
        ("numeric_string_exp", {"exp": "2000"}, 1100),
    ]
    claims = []
    for case, changes, now in variants:
        values = base | changes
        try:
            identity = _validate_claims(values, settings, {"nonce": "nonce"}, now)
            result = {"accepted": True, "identity": list(identity)}
        except (PermissionError, ValueError, TypeError):
            result = {"accepted": False}
        claims.append({"id": case, "claims": values, "now": now, "source": result})
    returns = [
        {"input": value, "source": safe_return_to(value)}
        for value in [
            None,
            "",
            "/workflows",
            "/workflows/research/a",
            "/workflows/research/a?x=1",
            "https://evil.example",
            "//evil.example/workflows",
            "/workflowsevil",
            "/workflows\\evil",
            "/workflows/a%5cb",
            "/workflows/%252f",
            "/workflows/../search",
            "/workflows/a\n",
        ]
    ]

    async def sessions() -> list[dict]:
        now = [1000.0]
        p = PortalPrincipal(
            "principal",
            "https://id.example",
            "subject",
            (
                PortalMembership("a", "Tenant A", frozenset({"workflow.read", "workflow.research.cancel"})),
                PortalMembership("b", "Tenant B", frozenset({"workflow.read"})),
            ),
        )
        directory = StaticMembershipResolver((p,))
        store = InMemoryStore()
        counter = iter(range(100))
        service = PortalSessionStore(
            store,
            directory,
            hmac_key=b"s" * 32,
            clock=lambda: now[0],
            token_source=lambda _: f"synthetic-{next(counter):032d}",
            idle_seconds=60,
            absolute_seconds=120,
        )

        def view(r):
            s = r.session
            return {
                "active": s.active_tenant if s else None,
                "generation": s.generation if s else None,
                "grants": sorted(r.context.grants) if r.context else None,
                "expired": r.expired,
                "revoked": r.revoked,
                "unavailable": r.unavailable,
            }

        out = []
        h, _ = await service.create(p, None)
        out.append({"id": "pending", "source": view(await service.resolve(h, frozenset({"workflow.read"})))})
        _, s = h, (await service.resolve(h, frozenset({"workflow.read"}))).session
        h2, _ = await service.rotate(h, s, "a")
        out.append({"id": "rotated", "source": view(await service.resolve(h2, frozenset({"workflow.read"})))})
        out.append({"id": "old_missing", "source": view(await service.resolve(h, frozenset({"workflow.read"})))})
        now[0] = 1120
        out.append({"id": "absolute_expired", "source": view(await service.resolve(h2, frozenset({"workflow.read"})))})
        now[0] = 1000
        h, _ = await service.create(p, "a")
        directory._by_id[p.principal_id] = PortalPrincipal(
            p.principal_id, p.issuer, p.subject, p.memberships, revision=2
        )
        out.append({"id": "revision_revoked", "source": view(await service.resolve(h, frozenset({"workflow.read"})))})
        out.append({"id": "revocation_missing", "source": view(await service.resolve(h, frozenset({"workflow.read"})))})
        store.is_connected = False
        out.append({"id": "unavailable", "source": view(await service.resolve(h, frozenset({"workflow.read"})))})
        return out

    # Source verifier is exercised only against its existing fixed public fixture.
    namespace = {}
    exec(compile((root / "tests/test_portal_oidc.py").read_bytes(), "test_portal_oidc.py", "exec"), namespace)
    token, jwks = namespace["TOKEN"], namespace["JWKS"]
    signature_cases = []
    for name, value in [("valid", token), ("tampered", token[:-1] + ("A" if token[-1] != "A" else "B"))]:
        try:
            result = _verify_rs256(value, jwks)
            expected = {"accepted": True, "claims": result}
        except PermissionError:
            expected = {"accepted": False}
        signature_cases.append({"id": name, "token": value, "jwks": jwks, "source": expected})
    directories = []
    good_directory = {
        "principals": [
            {
                "principal_id": "principal",
                "issuer": "https://id.example",
                "subject": "subject",
                "memberships": [
                    {
                        "tenant_id": "a",
                        "display_name": "A",
                        "grants": ["workflow.read", "workflow.research.cancel", "workflow.saved.control"],
                    }
                ],
            }
        ]
    }
    for name, change in [
        ("known_mixed", {}),
        ("unknown_grant", {"grants": ["workflow.unknown"]}),
        ("duplicate_grants", {"grants": ["workflow.read", "workflow.read"]}),
        ("bool_revision", {"revision": True}),
        ("zero_revision", {"revision": 0}),
        ("empty_grants", {"grants": []}),
    ]:
        data = json.loads(json.dumps(good_directory))
        data["principals"][0]["memberships"][0].update(change)
        with tempfile.TemporaryDirectory(prefix="ssx-t09-oracle-") as directory:
            location = Path(directory) / "members.json"
            location.write_text(json.dumps(data))
            try:
                parsed = _load_directory(str(location), settings.issuer)
                principal = asyncio.run(parsed.resolve_principal("principal"))
                expected = {"accepted": True, "grants": sorted(principal.memberships[0].grants)}
            except ValueError:
                expected = {"accepted": False}
        directories.append({"id": name, "input": data, "source": expected})
    settings_cases = []
    for name, issuer, callback in [
        ("valid", "https://id.example", "https://portal.example/auth/callback"),
        ("issuer_http", "http://id.example", "https://portal.example/auth/callback"),
        ("callback_http", "https://id.example", "http://portal.example/auth/callback"),
        ("callback_wrong_path", "https://id.example", "https://portal.example/other"),
        ("callback_query", "https://id.example", "https://portal.example/auth/callback?x=1"),
        ("issuer_userinfo", "https://user@id.example", "https://portal.example/auth/callback"),
    ]:
        try:
            OIDCSettings(issuer, "client", "synthetic-client-secret", callback)
            accepted = True
        except ValueError:
            accepted = False
        settings_cases.append({"id": name, "issuer": issuer, "callback": callback, "source": {"accepted": accepted}})
    tree = ast.parse((root / "slopsearx/workflow_portal.py").read_text())
    expressions = [
        node.value
        for node in ast.walk(tree)
        if isinstance(node, ast.Assign)
        and any(isinstance(t, ast.Name) and t.id == "digest" for t in node.targets)
        and "hashlib.sha256" in ast.unparse(node.value)
        and "strategy" in ast.unparse(node.value)
    ]
    if len(expressions) != 1:
        raise RuntimeError("source semantic mutation digest expression missing/ambiguous")
    expression = compile(ast.Expression(expressions[0]), "source-mutation-digest", "eval")
    mutation_digests = []
    for name, values in [
        ("default", {}),
        ("explicit_default", {"strategy": "triangulate"}),
        ("question_changed", {"question": "changed"}),
        ("unicode", {"question": "évidence", "object_id": "δοκιμή"}),
        ("cursor", {"cursor": "opaque-cursor"}),
    ]:
        fields = {
            "kind": "research",
            "object_id": "a-object",
            "action": "cancel",
            "revision": 1,
            "cursor": "",
            "question": "",
            "strategy": "triangulate",
        } | values
        mutation_digests.append({"id": name, "input": fields, "source": eval(expression, {"hashlib": hashlib}, fields)})
    previous_key_drain = []
    for raw in [
        "60",
        " 60 ",
        "+60",
        "6_0",
        "٦٠",
        "６０",
        "\u00a060\u00a0",
        "\u001c60\u001c",
        "60.0",
        "6e1",
        "6__0",
        "_60",
        "59",
        "601",
        "NaN",
    ]:
        with patch.dict(
            os.environ,
            {"SLOPSEARX_PORTAL_PREVIOUS_SESSION_KEY": "p" * 32, "SLOPSEARX_PORTAL_PREVIOUS_KEY_DRAIN_SECONDS": raw},
        ):
            try:
                result = _previous_secret()[1]
            except ValueError:
                result = None
        previous_key_drain.append({"raw": raw, "source": result})
    output = {
        "schema_version": 1,
        "baseline": BASELINE,
        "python": platform.python_version(),
        "provider_calls": 0,
        "exporter_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        "sources": sources,
        "claims": claims,
        "return_paths": returns,
        "sessions": asyncio.run(sessions()),
        "signatures": signature_cases,
        "directories": directories,
        "settings": settings_cases,
        "mutation_digests": mutation_digests,
        "previous_key_drain": previous_key_drain,
    }
    Path(sys.argv[2]).write_text(json.dumps(output, ensure_ascii=False, indent=2, allow_nan=False) + "\n")


if __name__ == "__main__":
    main()
