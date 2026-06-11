# PubChem

Search chemical compounds, substances, and bioassays. No auth required.

- **File:** `engines/pubchem.py`
- **Type:** API
- **Auth:** None
- **Categories:** general, science, reference, chemistry, medical
- **Rate limit:** 5 req/s
- **Base URL:** `https://pubchem.ncbi.nlm.nih.gov/rest/pug/compound/name/search`

## Usage

```
?q=<query>&engines=pubchem
?q=<query>&categories=chemistry
```

## Response

Returns chemical compound results with name, molecular formula, molecular weight, CID, synonyms, and structure image URL.

## Notes

- Free, public API from NCBI.
- Covers over 116M compounds, 310M substances, and 1.3M bioassays.
- Supports search by chemical name, CAS number, or SMILES.
