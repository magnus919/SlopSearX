# Pull request review operations

SlopSearX’s PR analysis runs from this repository and calls a pinned reusable workflow in [`groktopus/codereview`](https://github.com/groktopus/codereview). The target analysis workflow has read-only permissions. A separate target-side publisher validates the analysis artifact and, under the deployed v3 policy, can publish a `COMMENT` review using the workflow’s `GITHUB_TOKEN`. A successful analysis or publisher run alone does not prove that a review was published.

## Configure the reviewer endpoints

In the SlopSearX repository, open **Settings → Secrets and variables → Actions**. The workflow expects these six repository secrets:

| Secret | Purpose |
|---|---|
| `LLM_BASE_URL` | LLM provider endpoint |
| `LLM_MODEL` | LLM reviewer model identifier |
| `LLM_API_KEY` | Credential for the LLM endpoint |
| `JEV_BASE_URL` | Bounded classifier endpoint |
| `JEV_MODEL` | Bounded classifier model identifier |
| `JEV_API_KEY` | Credential for the bounded classifier endpoint |

Use the approved endpoint and model values for this deployment. The bounded classifier provides an advisory classification; deterministic runtime controls retain authority over coverage, disposition eligibility, and publication. Keep credentials in repository Actions secrets; do not put secret values in workflow files, variables, PRs, logs, or review artifacts. The caller passes these settings to the central workflow. The central workflow does not receive the target repository’s GitHub write token.

## Current canary and rollout boundary

The deployed v3 canary is comment-only. Its publisher policy allows `COMMENT`; it does not publish `APPROVE` or `REQUEST_CHANGES`. Droid Auto Review remains enabled during the canary. The separate `Droid Tag` and `Droid Wiki Refresh` workflows are independent and are not part of the planned cutover.

Two earlier v2 canary attempts ended with publication `UNKNOWN` after the publisher rejected artifact redirect hosts `productionresultssa9.blob.core.windows.net` and `productionresultssa14.blob.core.windows`, respectively. The first analysis reported `REQUEST_CHANGES` with `PARTIAL` coverage; the second reported `INCOMPLETE` with `PARTIAL` coverage and zero findings. These failures are historical evidence of host-pool churn, not an exhaustive list of storage hosts. The deployed v3 runtime removes the static storage-host list. For each already-bound artifact ID, the publisher uses the exact ephemeral `Location` returned by its authenticated GitHub artifact API request. It validates the HTTPS URL shape, uses the location only for that download, sends no GitHub bearer token to storage, and refuses a second redirect. It still verifies the API artifact digest, bounded archive size, manifest identity, and source-run bindings. Signed locations are not persisted or logged; do not guess storage hosts, add wildcard hosts, or accept a location from the artifact body. Confirm a review by checking the publisher receipt and the pull request’s review timeline. The v3 publication path requires a successful receipt/readback before its effect is considered confirmed; a green workflow alone is not proof.

The planned final stage is a separate change that may allow all three GitHub review dispositions and disable only Droid Auto Review. It must follow confirmation of the canary publication and readback. It does not enable automatic merging. If a run reports `UNKNOWN`, treat the effect as unconfirmed and inspect its analysis artifact, publisher result, and the PR timeline before proceeding.

## Read a run’s evidence

Open the **PR Review Analysis** run in Actions and inspect its result and provenance artifacts. Then inspect the corresponding protected publisher run and its effect receipt. Check that the repository, PR number, source run, and artifact bindings agree; read the reported disposition and coverage state; and confirm the resulting review on the PR itself. Keep `PARTIAL`, `UNKNOWN`, and missing evidence visible. Do not infer complete coverage or reviewer quality from a successful workflow status, a model disposition, or a published comment.

The current evidence establishes that the bounded workflow can analyze a real PR. It does not establish calibrated review quality or accuracy. Treat model output as advisory until separately supported by representative reviewed outcomes.

## Rollback

To contain future effects, first disable **PR Review Analysis** and the protected publisher workflow in the repository’s Actions settings. Then open a scoped PR restoring the prior policy and workflow guards and re-enabling Droid Auto Review. Review the diff against intervening changes; leave the separate Droid Tag and Wiki workflows untouched. Confirm both review jobs stay disabled until the restoration PR is merged. Rollback does not remove reviews already posted to pull requests. No workflow in this rollout merges a PR automatically.
