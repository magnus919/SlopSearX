"""Structural URL eligibility for retrieval handoffs; never resolves DNS or fetches.

The downstream retriever remains responsible for post-resolution SSRF checks.
This module has no tool handlers, runtime state, or dispatch dependencies.
"""

from __future__ import annotations

import ipaddress
import socket
from urllib.parse import urlparse

# Closed vocabulary of ``retrieval.url_status`` tokens. A URL is eligible for
# downstream retrieval only when the status is ``ok``; every other token is
# a machine-readable failure/warning reason for the handoff boundary.
RETRIEVAL_URL_STATUS_OK = "ok"
RETRIEVAL_URL_STATUS_MISSING = "missing"
RETRIEVAL_URL_STATUS_NON_HTTP = "non_http"
RETRIEVAL_URL_STATUS_UNSAFE = "unsafe_scheme"
RETRIEVAL_URL_STATUS_AMBIGUOUS = "ambiguous"
RETRIEVAL_URL_STATUSES: tuple[str, ...] = (
    RETRIEVAL_URL_STATUS_OK,
    RETRIEVAL_URL_STATUS_MISSING,
    RETRIEVAL_URL_STATUS_NON_HTTP,
    RETRIEVAL_URL_STATUS_UNSAFE,
    RETRIEVAL_URL_STATUS_AMBIGUOUS,
)

# Schemes an HTTP-based downstream retriever must never be pointed at. This is
# advisory classification at the handoff boundary — SlopSearX performs no fetch
# itself — so a downstream capture layer can reject these URLs without
# becoming an SSRF-capable proxy or mishandling embedded content.
UNSAFE_RETRIEVAL_SCHEMES = frozenset({"file", "data", "javascript", "vbscript", "gopher", "ftp"})

# Highest port accepted for a handoff fetch target. Anything above this (or
# non-numeric) is classified ``ambiguous`` and never handed off; port ``0`` is
# excluded because it is not a connectable fetch target.
RETRIEVAL_PORT_MAX = 65535

# RFC 3879 deprecated the IPv6 site-local addressing scope (fec0::/10).
# ``ipaddress`` classifies the range neither reserved nor private on modern
# CPython, so a site-local literal reports is_global=True and would slip past
# the globality guard. It is not globally routable, so any literal in this
# prefix is never handed off as a fetch target.
RETRIEVAL_DEPRECATED_SITE_LOCAL_V6 = ipaddress.ip_network("fec0::/10")

# RFC 3056 6to4 embeds the IPv4 in the low 32 bits of every ``2002::/16``
# address, so a 6to4 literal can resolve to loopback
# (``http://[2002:7f00:1::1]/`` -> 127.0.0.1) or cloud metadata
# (``http://[2002:a9fe:a9fe::]/`` -> 169.254.169.254) at fetch time.
# CPython only added ``2002::/16`` to ``IPv6Address._private_networks`` in
# 3.12.3, so on the ``>=3.12`` floor (3.12.0-3.12.2) a 6to4 literal reports
# is_global=True and slips past the globality guard — a 6to4-embedded IPv4
# SSRF (CWE-918). The prefix is not globally routable as a literal host, so
# it is rejected explicitly on every version.
RETRIEVAL_SIXTOFOUR_V6 = ipaddress.ip_network("2002::/16")


def _ipv4_component_value(part: str) -> int:
    """Parse one WHATWG IPv4 component (decimal/hex/octal) or raise ValueError.

    WHATWG URL clients infer the component base from its prefix: ``0x``/``0X``
    is hexadecimal, a leading ``0`` is octal (so ``0177`` is 127, not 177),
    and everything else is decimal. Python's ``int``/``ipaddress`` do not
    apply these rules, so a host like ``0177.0.0.1`` would otherwise evade a
    literal-IP guard. A bare ``0x``/``0X`` component is treated as zero
    (Chromium behavior), which keeps ``http://0x/`` (-> 0.0.0.0) inside the
    guard.
    """
    if not part:
        raise ValueError("empty IPv4 component")
    if part[:2] in ("0x", "0X"):
        digits = part[2:]
        if not digits:
            return 0
        base = 16
        allowed = "0123456789abcdefABCDEF"
    elif part.startswith("0"):
        digits = part
        base = 8
        allowed = "01234567"
    else:
        digits = part
        base = 10
        allowed = "0123456789"
    if any(char not in allowed for char in digits):
        raise ValueError("IPv4 component is not numeric in its base")
    value = int(digits, base)
    if value > 0xFFFFFFFF:
        raise ValueError("IPv4 component exceeds 32 bits")
    return value


def _whatwg_ipv4_literal(host: str) -> ipaddress.IPv4Address | None:
    """Resolve a host as a WHATWG-style IPv4 literal, or return ``None``.

    Mirrors the WHATWG URL host parser's IPv4 rules (verified against
    Chromium): at most four dot-separated components parsed as hex/octal/
    decimal, abbreviated forms (``127.1``), a trailing dot, and a last
    component that may carry the remaining 8-24 bits. Returns the address a
    WHATWG client would resolve, or ``None`` when the host is not an IPv4
    literal (a normal hostname). No DNS is performed.
    """
    parts = host.split(".")
    if parts and parts[-1] == "":
        parts = parts[:-1]
    if not parts or len(parts) > 4:
        return None
    numbers: list[int] = []
    for part in parts:
        try:
            numbers.append(_ipv4_component_value(part))
        except ValueError:
            return None
    if len(numbers) == 1:
        return ipaddress.IPv4Address(numbers[0])
    if numbers[0] > 255:
        return None
    for number in numbers[1:-1]:
        if number > 255:
            return None
    if numbers[-1] >= 256 ** (5 - len(numbers)):
        return None
    ipv4 = numbers[-1]
    for index in range(len(numbers) - 2, -1, -1):
        ipv4 += numbers[index] * (256 ** (3 - index))
    return ipaddress.IPv4Address(ipv4)


def _ip_literal_candidates(host: str) -> list[ipaddress.IPv4Address | ipaddress.IPv6Address]:
    """Literal IP address(es) a host denotes, with no DNS resolution.

    ``urlparse`` reports the host verbatim, but WHATWG clients (browsers,
    HTTP stacks) accept numeric host forms that ``ipaddress.ip_address``
    rejects: integer literals (``2130706433``), hex (``0x7f000001``), octal
    (``0177.0.0.1``), and abbreviated dotted forms (``127.1``). Candidates
    come from three deterministic sources: canonical ``ipaddress`` parsing,
    the WHATWG-style IPv4 parser above, and ``getaddrinfo`` restricted to
    ``AI_NUMERICHOST`` (a numeric-only flag that never triggers DNS). A host
    that yields no candidates is not a parseable numeric literal: a genuine
    hostname (one containing non-numeric characters) classifies normally,
    while a dotted all-numeric host that fails every candidate parser (e.g.
    ``169..127.1`` or ``1.2.3.300``) is treated as a malformed IPv4 literal
    attempt, never as a normal hostname.
    """
    candidates: list[ipaddress.IPv4Address | ipaddress.IPv6Address] = []
    try:
        candidates.append(ipaddress.ip_address(host))
    except ValueError:
        pass
    ipv4 = _whatwg_ipv4_literal(host)
    if ipv4 is not None:
        candidates.append(ipv4)
    try:
        infos = socket.getaddrinfo(host, None, type=socket.SOCK_STREAM, flags=socket.AI_NUMERICHOST)
    except (socket.gaierror, ValueError, UnicodeError, OverflowError):
        infos = []
    for info in infos:
        try:
            candidates.append(ipaddress.ip_address(info[4][0]))
        except ValueError:
            continue
    return list(dict.fromkeys(candidates))


def _retrieval_url(url: str) -> tuple[str, str | None, str | None, str | None]:
    """Classify a result URL for downstream retrieval — never fetches it.

    SlopSearX is a search-only service: this is advisory metadata so a
    downstream retriever (e.g. GroktoCrawl) can decide eligibility and fetch
    safety without parsing prose. Returns ``(status, reason, scheme, url)``
    where ``status`` is one of ``RETRIEVAL_URL_STATUSES`` and the URL is
    non-``None`` **only** for ``ok`` — ineligible URLs are never handed off
    as a fetch target.

    - ``ok`` — absolute ``http``/``https`` URL with a well-formed authority
      (non-empty host, no backslash, whitespace, or control characters,
      valid in-range port); eligible, and the captured URL is handed off
      verbatim (never canonicalized or rewritten). ``ok`` is a
      literal/structural certification only: no DNS resolution is performed,
      so a DNS-resolvable hostname — including nip.io-style aliases of
      loopback/link-local/metadata IPs (``169.254.169.254.nip.io``,
      ``127.0.0.1.nip.io``) and ``localtest.me`` — is certified ``ok`` even
      though it may resolve to a private or link-local address at fetch time.
      The downstream retriever MUST enforce its own post-resolution SSRF
      controls (including blocking DNS-rebinding and nip.io-style aliases).
    - ``missing`` — no URL string on the result.
    - ``non_http`` — parses to a non-HTTP(S) scheme (e.g. ``mailto:``).
    - ``unsafe_scheme`` — a scheme an HTTP retriever must not fetch
      (``file:``, ``data:``, ``javascript:``, ...); see
      ``UNSAFE_RETRIEVAL_SCHEMES``.
    - ``ambiguous`` — not a safe fetch target: canonicalization-ambiguous
      (no scheme, no host, a backslash, whitespace, or control character in
      the authority, a percent-encoded or non-ASCII host, an invalid or
      out-of-range port, or unparseable), a literal IP host — in any numeric
      encoding, including decimal/hex/octal/abbreviated forms — that is not
      globally routable or that lives in a reserved or translation-prefix
      class (RFC 6052 NAT64 ``64:ff9b::/96``, the IPv4-translatable
      ``::ffff:0:0:0/96``, IPv4-mapped literals embedding a non-global IPv4,
      the RFC 3879 deprecated site-local ``fec0::/10``, or the RFC 3056 6to4
      ``2002::/16``), a dotted all-numeric host no literal-IP parser can
      decode (an empty or out-of-range octet, e.g. ``169..127.1`` or
      ``1.2.3.300``), or an authority carrying userinfo credentials; treated
      as ineligible rather than guessed at.
    """
    if not url or not url.strip():
        return RETRIEVAL_URL_STATUS_MISSING, "result has no URL to retrieve", None, None
    try:
        parsed = urlparse(url)
        scheme = (parsed.scheme or "").lower()
        if not scheme:
            return RETRIEVAL_URL_STATUS_AMBIGUOUS, "URL has no scheme; canonicalization is ambiguous", None, None
        if scheme not in ("http", "https"):
            if scheme in UNSAFE_RETRIEVAL_SCHEMES:
                return (
                    RETRIEVAL_URL_STATUS_UNSAFE,
                    f"scheme '{scheme}' is unsafe for downstream HTTP retrieval",
                    scheme,
                    None,
                )
            return (
                RETRIEVAL_URL_STATUS_NON_HTTP,
                f"scheme '{scheme}' is not HTTP(S); not retrievable over HTTP",
                scheme,
                None,
            )
        if not parsed.hostname:
            return RETRIEVAL_URL_STATUS_AMBIGUOUS, "URL has no host; canonicalization is ambiguous", scheme, None
        # urlparse does not normalize backslashes and does not strip control
        # characters or whitespace, so the authority it reports is not
        # necessarily the authority a WHATWG client (browser/HTTP stack)
        # would resolve: e.g. "https://internal.example\@public.com/" parses
        # with hostname "public.com" here but a WHATWG client connects to
        # "internal.example", and a literal space in the authority would
        # otherwise certify "http:// example.com/" as fetchable. Never hand
        # off a target whose authority cannot be canonicalized unambiguously
        # (CWE-918 host-confusion guard).
        if "\\" in parsed.netloc or any(
            ord(char) < 0x20 or ord(char) == 0x7F or char.isspace() for char in parsed.netloc
        ):
            return (
                RETRIEVAL_URL_STATUS_AMBIGUOUS,
                "URL authority contains a backslash, whitespace, or control character; canonicalization is ambiguous",
                scheme,
                None,
            )
        # urlparse does not reject credentials embedded in the authority; a
        # URL like "http://user:pass@example.com/" would otherwise pass every
        # check and be returned verbatim as the fetch target, persisting
        # credentials and causing downstream Basic-auth transmission. Any
        # userinfo in the authority makes the target ineligible.
        if parsed.username is not None or parsed.password is not None:
            return (
                RETRIEVAL_URL_STATUS_AMBIGUOUS,
                "URL authority contains userinfo credentials; not handed off as a fetch target",
                scheme,
                None,
            )
        # urlparse does not percent-decode or IDNA-map the host, but WHATWG
        # clients (browsers, HTTP stacks) do, so a percent-encoded host
        # ("%31%36%39.%32%35%34..." -> 169.254.169.254) or a fullwidth host
        # would be certified ok here while a WHATWG client resolves a
        # different host. Never hand off a target whose host cannot be
        # canonicalized unambiguously (CWE-918 host-confusion guard).
        if "%" in parsed.hostname or any(ord(char) > 0x7F for char in parsed.hostname):
            return (
                RETRIEVAL_URL_STATUS_AMBIGUOUS,
                "URL host contains a percent-encoded or non-ASCII character; canonicalization is ambiguous",
                scheme,
                None,
            )
        # A literal IP host — in any numeric encoding, not just canonical
        # dotted-quad — is not a safe fetch target for a downstream retriever
        # (SSRF: http://127.0.0.1/, http://[::1]/, http://10.0.0.1/, the cloud
        # metadata IP http://169.254.169.254/latest/meta-data/, and
        # non-canonical forms like http://2130706433/, http://127.1/,
        # http://0177.0.0.1/, or http://0x7f000001/ that WHATWG clients
        # resolve to loopback). Only literal IPs are checked — getaddrinfo is
        # used solely with AI_NUMERICHOST, so no DNS resolution is performed —
        # and any non-global address (loopback, private, CGNAT, link-local,
        # reserved, documentation, or unspecified) is never handed off. Four
        # additional classes are rejected because ``ipaddress.is_global``
        # alone reports them global: reserved IPv6 prefixes with no embedded
        # IPv4 (``ipv4_mapped is None`` — RFC 6052 NAT64 ``64:ff9b::/96``
        # and the IPv4-translatable ``::ffff:0:0:0/96`` report is_global=True
        # regardless of the IPv4 they embed), the RFC 3879 deprecated
        # site-local scope (``fec0::/10``, neither reserved nor private), the
        # RFC 3056 6to4 prefix (``2002::/16``, is_global=True on CPython
        # 3.12.0-3.12.2 while the embedded IPv4 can still be loopback or
        # metadata), and IPv4-mapped literals (``::ffff:0:0/96``) whose
        # embedded IPv4 is not globally reachable. IPv4-mapped literals
        # (``::ffff:0:0/96``) are judged by their embedded IPv4 instead: a
        # public embedded address is eligible, while an embedded address that
        # ``ipaddress`` does not classify as globally reachable (loopback,
        # private, link-local, CGNAT, unspecified, reserved, documentation)
        # is never handed off. A dotted all-numeric host that every candidate
        # parser rejects (an empty or out-of-range octet, e.g. "169..127.1"
        # or "1.2.3.300") is a failed IPv4 literal attempt, not a legitimate
        # hostname — a WHATWG client would fail to canonicalize it, so it is
        # never certified ok.
        ip_candidates = _ip_literal_candidates(parsed.hostname)
        for ip in ip_candidates:
            # Reject the embed-agnostic classes first, each with a distinct
            # reserved/translation-prefix reason: the RFC 3056 6to4 prefix
            # (``2002::/16`` — is_global=True on CPython 3.12.0-3.12.2 while
            # the embedded IPv4 can still be loopback or metadata), the RFC
            # 3879 deprecated site-local scope (``fec0::/10``, is_global=True
            # on every supported version), and IPv4-mapped literals
            # (``::ffff:0:0/96``) whose embedded IPv4 is not globally
            # reachable (loopback, private, link-local, CGNAT, unspecified,
            # reserved, documentation). These are blocked for being reserved
            # or translation prefixes (or embedding one), not for being
            # non-global, so they must not be labelled with the non-global
            # reason.
            if (
                (ip.version == 6 and ip in RETRIEVAL_SIXTOFOUR_V6)
                or (ip.version == 6 and ip in RETRIEVAL_DEPRECATED_SITE_LOCAL_V6)
                or (ip.version == 6 and ip.ipv4_mapped is not None and not ip.ipv4_mapped.is_global)
            ):
                return (
                    RETRIEVAL_URL_STATUS_AMBIGUOUS,
                    "URL host is a reserved or translation-prefix IP address; not a safe fetch target",
                    scheme,
                    None,
                )
            # Genuinely non-global addresses (loopback, private, CGNAT,
            # link-local, documentation, unspecified) keep the non-global
            # reason. This branch runs before the reserved-prefix check so a
            # loopback like ``::1`` — which ``ipaddress`` marks reserved too —
            # is still reported as non-global.
            if not ip.is_global:
                return (
                    RETRIEVAL_URL_STATUS_AMBIGUOUS,
                    "URL host is a non-global literal IP address; not a safe fetch target",
                    scheme,
                    None,
                )
            # Reserved IPv6 prefixes with no embedded IPv4 (``ipv4_mapped is
            # None`` — RFC 6052 NAT64 ``64:ff9b::/96`` and the IPv4-translatable
            # ``::ffff:0:0:0/96``) report is_global=True regardless of the
            # IPv4 they embed, so only this reserved-prefix check catches
            # them; they are blocked for being reserved prefixes, hence the
            # reserved/translation-prefix reason.
            if ip.version == 6 and ip.ipv4_mapped is None and ip.is_reserved:
                return (
                    RETRIEVAL_URL_STATUS_AMBIGUOUS,
                    "URL host is a reserved or translation-prefix IP address; not a safe fetch target",
                    scheme,
                    None,
                )
        if not ip_candidates and all(char.isdigit() or char == "." for char in parsed.hostname):
            return (
                RETRIEVAL_URL_STATUS_AMBIGUOUS,
                "URL host is a malformed dotted numeric literal; canonicalization is ambiguous",
                scheme,
                None,
            )
        # A port that is not a parseable integer or is outside the sane TCP
        # range is not a fetchable target (http://host:abc/,
        # http://host:99999/, http://host:0/).
        try:
            port = parsed.port
        except ValueError:
            return (
                RETRIEVAL_URL_STATUS_AMBIGUOUS,
                "URL port is invalid; canonicalization is ambiguous",
                scheme,
                None,
            )
        if port is not None and not 1 <= port <= RETRIEVAL_PORT_MAX:
            return (
                RETRIEVAL_URL_STATUS_AMBIGUOUS,
                f"URL port is outside the sane range (1-{RETRIEVAL_PORT_MAX}); canonicalization is ambiguous",
                scheme,
                None,
            )
        return RETRIEVAL_URL_STATUS_OK, None, scheme, url
    except ValueError:
        # urlparse is lenient but can raise for malformed bracketed hosts
        # (e.g. "http://[::1"); access to .hostname can also raise.
        return RETRIEVAL_URL_STATUS_AMBIGUOUS, "URL cannot be parsed unambiguously", None, None


def classify_retrieval_url(url: str) -> tuple[str, str | None, str | None, str | None]:
    """Expose the shared structural handoff classification to non-MCP views."""
    return _retrieval_url(url)
