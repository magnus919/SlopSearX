# Reddit

Search Reddit posts and comments via the official JSON API. No auth required.

- **File:** `engines/reddit.py`
- **Type:** API
- **Auth:** None
- **Categories:** general, social, reddit:subreddit
- **Rate limit:** 1 req/s (~60 req/min)
- **Base URL:** `https://www.reddit.com`

## Usage

```
?q=<query>&engines=reddit
?q=<query>&categories=social
?q=<query>&categories=reddit:subreddit
```

## Sub-Category: reddit:subreddit

When using the `reddit:subreddit` sub-category, the adapter searches within a specific subreddit. The subreddit name is passed via `params["subreddit"]`.

## Response

Returns Reddit posts with titles, URLs, scores, subreddit, author, comment count, and timestamps.
