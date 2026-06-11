# openFDA

Search FDA drug, device, and food recall data. No auth required.

- **File:** `engines/openfda.py`
- **Type:** API
- **Auth:** None
- **Categories:** general, medical, health, science, government
- **Rate limit:** 240 req/min
- **Base URL:** `https://api.fda.gov/drug/label.json`

## Usage

```
?q=<query>&engines=openfda
?q=<query>&categories=medical
```

## Response

Returns FDA-regulated product results with brand name, generic name, manufacturer, purpose, warnings, and active ingredients.

## Notes

- Free, public API from the US Food and Drug Administration.
- Covers drug labels, adverse events, recall enforcement, and device registrations.
