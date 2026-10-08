# EXP-105: carry summary contract correction

EXP-104 passed the service health probe but stopped during initialization because its adapter omitted `quality_analyzed: false` from the verified carry summary. No grader was called. The stopped clock is retained and cannot resume.

This prospective correction restores the summary required by the unchanged admission gate. It preserves all 28 accepted EXP-103 closures and admits only four missing original d8 assessments after source qualification, independent review, merge and a new clock and health check. Historical grader and combined charges remain 103 and 121; planned totals remain 107 and 125. No new search, Jev, source or answerer calls are authorized. Quality analysis and production adoption remain unproven.

The public regression uses the original admission implementation, including rejection of the old summary. Qualification protects inherited source files, both old seals, original inputs and accepted carry. Private operational artifacts are excluded.
