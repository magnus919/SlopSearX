# Test ranking without repeating development searches

This method uses the complete saved development search results and page captures
from the earlier run. That run stopped before ranking or answer generation.
Its old grades do not carry forward, and it receives no quality credit.

The new development run verifies the original files and failure records, then
creates fresh blind grading packets. It makes no new search or page-capture
requests. Missing pages remain missing; simulated content cannot replace them.
The original collection and capture costs remain visible alongside the new
run's model calls and elapsed time.

The ranking candidate, quality thresholds, model aliases, usage ceilings, and
no-retry rule are unchanged. Confirmation uses separate cases and fresh search
results and captures, and runs only after every development check passes.

The engine replay tool tests adapters and failure handling offline. Its results
cannot establish ranking quality or count toward either scientific run.

The files here define the method. Private registrations bind each actual run to
its source revision, new run identities, and the verified original archive.
This bundle does not authorize a production deployment.
