# EXP-046 offline qualification

The runner and fixtures are frozen against registration merge `7943def68cf8184cedf1a5afcdbb8bba146de8c1`. No provider or search calls were made during qualification. Brave use remains 0/10.

Root reran the exact publication bytes: 21 author fixtures, independent Decimal/boundary/raw-action checks, eight helper groups, publication-gate checks, the complete synthetic 21-operation receipt-to-analysis path with 11 mutation/stop cases, and 14 offline transport cases passed. Historical integrity traversal verified 395 file comparisons across 289 unique files and 12 current manifests. Archived qualification-v1 bytes are checked through their current outer manifest; their historical relative paths are not treated as current directories. The transport credential value in its fixture is explicitly synthetic.

The complete fake run reaches both references, eight primary query deltas per reference, and all 13 pool inventories. Synthetic KEEP responses fail the quality gate as expected. These checks establish harness behavior, not usefulness, calibration, provider acceptance, or production readiness. The real synthetic neutral acceptance request remains unattempted and must be separately published before development calls.

Retained intermediate probes include the expected rejection before the Decimal implementation and the full-run receipt projection mismatch while files were still being edited. The latter records the source fingerprint observed after the run, rather than claiming an immutable tested snapshot. EXP-045's actual protocol failure remains unchanged.

Generic SlopSearX API/MCP/portal/cache integration and experimental GroktoCrawl X forwarding remain open and require qualified development, fresh confirmation, runtime tests, review, and merge.

Independent review and root replay also completed the actual CLI sequence: prepare → neutral → 21 synthetic case operations → analyze, with local transport and publication checks mocked. The script and byte-identical independently reproduced output are retained. No blocking offline review finding remains. The self-test experiment label was corrected before final hashing and the entire final suite was rerun.
