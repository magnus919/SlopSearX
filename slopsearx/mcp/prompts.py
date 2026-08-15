"""MCP prompts — repeatable agent workflows (design §5).

Concise templates that invoke the tools rather than embedding engine
knowledge. Registered on the FastMCP instance by ``slopsearx.mcp.server``.
"""

from __future__ import annotations


def research_with_source_coverage(question: str) -> str:
    """Search broadly, inspect capability coverage, return diverse evidence."""
    return (
        f"Research question: {question}\n\n"
        "1. Call slopsearx_explain_search_scope to preview routing for this question.\n"
        "2. Call slopsearx_search with intent='auto' and inspect engine_outcomes.\n"
        "3. If coverage is thin, call slopsearx_list_capabilities to find source families, "
        "then slopsearx_search_targeted on the relevant engines.\n"
        "4. Report which sources responded and which failed; flag partial results.\n"
        "5. Cite results with their citation.url; never claim SlopSearX verified page bodies."
    )


def investigate_vulnerability(target: str) -> str:
    """Search security sources, separate discovery from confirmation."""
    return (
        f"Investigate security posture for: {target}\n\n"
        "1. Call slopsearx_search_security with evidence_types=['vulnerability', 'exposure', 'threat_intel'].\n"
        "2. Distinguish discovered mentions from confirmed findings; never equate absence in "
        "search results with absence of a vulnerability.\n"
        "3. Report which source families responded and which are missing (e.g. NVD vs vendor advisories).\n"
        "4. Do not present search snippets as a completed security assessment."
    )


def find_company_jobs(company: str) -> str:
    """Search ATS boards for a named company, preserving provenance."""
    return (
        f"Find open roles at: {company}\n\n"
        "1. Call slopsearx_search_jobs with company and relevant keywords.\n"
        "2. Preserve each result's source engine and any publication/update timestamp.\n"
        "3. If no board responded, say so explicitly — a missing board is not 'no jobs'.\n"
        "4. Note that results are search findings: no full job descriptions are available."
    )


def compare_package_or_project(name: str) -> str:
    """Search package registries and developer sources, dedupe by URL."""
    return (
        f"Compare the package/project: {name}\n\n"
        "1. Call slopsearx_search with intent='packages' (or 'code').\n"
        "2. Deduplicate by canonical URL and note which sources corroborate each result.\n"
        "3. Report which registries responded and which did not.\n"
        "4. Cite sources; do not claim SlopSearX verified the project's maintenance status."
    )
