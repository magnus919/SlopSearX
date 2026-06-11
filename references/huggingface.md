# HuggingFace

Search HuggingFace Hub: models, datasets, and papers. Optional HF token for higher rate limits.

- **File:** `engines/huggingface.py`
- **Type:** API
- **Auth:** `ENGINE_HUGGINGFACE_API_KEY` (optional)
- **Categories:** general, science, huggingface:datasets, huggingface:papers
- **Rate limit:** 1 req/s (without token), higher with token
- **Base URL:** `https://huggingface.co/api`

## Usage

```
?q=<query>&engines=huggingface
?q=<query>&categories=huggingface:datasets
?q=<query>&categories=huggingface:papers
```

## Sub-Categories

- `huggingface:datasets` — Search datasets on the Hub
- `huggingface:papers` — Search papers on the Hub
- Default — Search models on the Hub

## Response

Returns model/dataset/paper results with name, description, downloads, likes, tags, and last modified date.

## Notes

- Public API with generous rate limits; token increases limits.
- Token is optional but recommended for heavier usage.
