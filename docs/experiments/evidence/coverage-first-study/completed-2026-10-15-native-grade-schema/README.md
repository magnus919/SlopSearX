# Reference grading stopped on an invalid response

Development imported the saved real corpus and completed all 53 required native grading calls. The grading dispatcher returned successfully, but the independent grade validator rejected one source assessment response: it omitted required source and facet assessments and used a source identifier outside its assigned packet. The other 52 responses passed JSON Schema validation; their substantive judgments have not been qualified.

The study closed as incomplete after 1,964.72 seconds. It issued no ranking or answer calls and started no confirmation. Development made no new searches or captures. This is not evidence for or against the ranking candidate.

The complete original packets, blind assignments, responses and native invocation evidence are preserved privately. No response was edited or retried. These stage identities are retired. Production remains unchanged.

The adapter had sent the schema only as prompt text. The repair passes the exact packet schema through the CLI structured-output field, verifies that binding in the invocation transcript, and validates returned JSON before publishing it. Invalid results stop new dispatch rather than being reported as completed grades.
