# Fun facts

## Most files changed in a single commit

The initial commit (893dcbd) contained the entire project skeleton. It is the largest single commit by file count in the repository history.

## Internet Archive adapter routing

The Internet Archive adapter routes queries differently based on whether the query looks like a domain name or not. If the query resembles a domain (e.g. `example.com`), it routes to the Wayback CDX API for historical snapshots. Otherwise, it routes to the general archive search endpoint.

## Metrics implemented from scratch

The metrics module uses zero external dependencies. It implements Counter, Gauge, and Histogram classes from scratch in pure Python with no Prometheus client libraries or other metric-collection packages.

## 12 search engines out of the box

SlopSearX supports 12 search engines covering major platforms (Google, GitHub), privacy-focused providers (DuckDuckGo), developer platforms (Hacker News, Stack Exchange), and specialized academic databases (arXiv, Semantic Scholar, OpenAlex).

## Compact Docker image

The Docker image targets approximately 200MB with a cold start under 2 seconds. There are no headless browsers or Playwright dependencies, keeping the image lean and fast to boot.
