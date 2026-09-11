from __future__ import annotations

# ruff: noqa: E501 -- fixed cryptographic test vectors are intentionally unsplit internally.
import httpx
import pytest

from slopsearx.mcp.harness import InMemoryStore
from slopsearx.portal_oidc import OIDCIdentityProvider, OIDCSettings, _validate_claims, _verify_rs256

# Fixed public test vector. It contains no credential or production identity.
TOKEN = (
    "eyJhbGciOiJSUzI1NiIsImtpZCI6InRlc3QiLCJ0eXAiOiJKV1QifQ."
    "eyJpc3MiOiJodHRwczovL2lkLmV4YW1wbGUiLCJhdWQiOiJjbGllbnQiLCJzdWIiOiJzdWJqZWN0Iiwibm9uY2UiOiJub25jZSIsImlhdCI6MTAwMCwiZXhwIjoyMDAwfQ."
    "vwRodz902dFGrcblAYUGuZaPyG2DM4oDD4P0qmrXevtP8shDqjcIiUlQ27SOnr6LvVaf6-RhRvoTwOuHaHg_lH_zJndj7GohUKc7v8GPE4p03p8lQvFrgDJ2FA5P1yKcRyuaoBA3Z4szKuyTwcS1W_zaYzPVsnnvoUazWfM4CYa26f_-Kc5AMIIEk-Wz2ROSArLx-GEaOiqXUxsfSdYTzoc9sRCU3Fgjna1GsX86ql3r1NOMRszKcWnJBNanXFNm7PNBXWaKDAjCH8WRaMiE1-2CbFjw3_dLt2nFobMDt086Z0j9DKdbW7zERzlw8We3gmqkoopSvys8mEbNB1aZxw"
)
JWKS = {
    "keys": [
        {
            "kty": "RSA",
            "kid": "test",
            "use": "sig",
            "n": (
                "3OFy4SYFCE4erT6hGfGmW9ruy2-Yty_0aUy6TGATQPcceUBwAyEELZr0dkSIpVziwnSAAj3Rxe8c6eHPnM7PSQzA6q5wNVey93uvzssVaAdVdisks0-yc3FKLFW5Z5olk8ozzHJ5CU9zh-WDPhmjSvKFnrsUlxyAAWC7RK8r9mNsprgJIq0jJfwb-q0rL53YmmLlTuyro1JF8HoYgA1-eVGBKhbIgOS6356VqxRwjfKDcmMmFMz9YC7Awch-bYlgH3p9tSeHxw66LQx-p_r_Ww0WiYi1XR2yS8SI9MiSnzW4JwjF3pI6Ns2kPWrDGd4qaBpw0v_MvvNxN9iZbdkPnw"
            ),
            "e": "AQAB",
        }
    ]
}


def test_rs256_verifier_rejects_tampering() -> None:
    assert _verify_rs256(TOKEN, JWKS)["sub"] == "subject"
    with pytest.raises(PermissionError):
        _verify_rs256(TOKEN[:-1] + ("A" if TOKEN[-1] != "A" else "B"), JWKS)


async def test_oidc_pkce_state_nonce_and_one_time_callback() -> None:
    def handler(request: httpx.Request) -> httpx.Response:
        if request.url.path.endswith("openid-configuration"):
            return httpx.Response(
                200,
                json={
                    "issuer": "https://id.example",
                    "authorization_endpoint": "https://id.example/authorize",
                    "token_endpoint": "https://id.example/token",
                    "jwks_uri": "https://id.example/jwks",
                    "code_challenge_methods_supported": ["S256"],
                },
            )
        if request.url.path == "/token":
            assert b"code_verifier=verifier" in request.content
            return httpx.Response(200, json={"id_token": TOKEN})
        if request.url.path == "/jwks":
            return httpx.Response(200, json=JWKS)
        raise AssertionError(request.url)

    values = iter(("state", "nonce", "verifier", "transaction"))
    store = InMemoryStore()
    provider = OIDCIdentityProvider(
        OIDCSettings("https://id.example", "client", "secret", "https://portal.example/auth/callback"),
        store,
        transaction_key=b"k" * 32,
        client=httpx.AsyncClient(transport=httpx.MockTransport(handler)),
        clock=lambda: 1100.0,
        token_source=lambda _size: next(values),
    )
    started = await provider.begin("https://evil.example")
    assert "code_challenge_method=S256" in started.location
    assert "verifier" not in started.location
    with pytest.raises(PermissionError):
        await provider.complete(
            "https://portal.example/auth/callback?code=authorization-code&state=attacker", started.transaction_handle
        )
    result = await provider.complete(
        "https://portal.example/auth/callback?code=authorization-code&state=state", started.transaction_handle
    )
    assert result == ("https://id.example", "subject", "/workflows")
    with pytest.raises(PermissionError):
        await provider.complete(
            "https://portal.example/auth/callback?code=authorization-code&state=state", started.transaction_handle
        )


async def test_callback_transaction_is_atomic_and_duplicate_parameters_are_rejected() -> None:
    store = InMemoryStore()
    provider = OIDCIdentityProvider(
        OIDCSettings("https://id.example", "client", "secret", "https://portal.example/auth/callback"),
        store,
        transaction_key=b"k" * 32,
    )
    key = provider._transaction_key("handle")
    expected = {"created_at": 1000, "state": "state"}
    await store.set(key, expected, 600)
    first_read, second_read = await __import__("asyncio").gather(
        provider._read_transaction(key), provider._read_transaction(key)
    )
    first, second = await __import__("asyncio").gather(
        provider._consume_transaction(key, first_read[0] or {}, first_read[1]),
        provider._consume_transaction(key, second_read[0] or {}, second_read[1]),
    )
    assert sum((first, second)) == 1

    await store.set(key, {"created_at": provider._clock(), "state": "state"}, 600)
    with pytest.raises(PermissionError):
        await provider.complete("https://portal.example/auth/callback?code=one&code=two&state=state", "handle")
    assert await store.get(key) is not None


def test_multiple_audiences_require_authorized_party() -> None:
    settings = OIDCSettings("https://id.example", "client", "secret", "https://portal.example/auth/callback")
    claims = {
        "iss": "https://id.example",
        "aud": ["client", "other"],
        "sub": "subject",
        "nonce": "nonce",
        "iat": 1000,
        "exp": 2000,
    }
    with pytest.raises(PermissionError):
        _validate_claims(claims, settings, {"nonce": "nonce"}, 1100)
    assert _validate_claims({**claims, "azp": "client"}, settings, {"nonce": "nonce"}, 1100) == (
        "https://id.example",
        "subject",
    )


def test_oidc_settings_require_exact_https_callback() -> None:
    with pytest.raises(ValueError):
        OIDCSettings("http://id.example", "client", "secret", "https://portal.example/auth/callback")
    with pytest.raises(ValueError):
        OIDCSettings("https://id.example", "client", "secret", "https://portal.example/other")
    with pytest.raises(ValueError):
        OIDCSettings("https://id.example", "client", "secret", "https://portal.example/auth/callback?next=x")
    with pytest.raises(ValueError):
        OIDCSettings("https://id.example", "client", "secret", "https://portal.example/auth/callback#fragment")
