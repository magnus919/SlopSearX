# UniProt

Search protein sequence and function data. No auth required.

- **File:** `engines/uniprot.py`
- **Type:** API
- **Auth:** None
- **Categories:** general, science, reference, biology, medical
- **Rate limit:** None published
- **Base URL:** `https://rest.uniprot.org/uniprotkb/search`

## Usage

```
?q=<query>&engines=uniprot
?q=<query>&categories=biology
```

## Response

Returns protein results with accession ID, name, organism, gene, function description, and sequence length.

## Notes

- Free, public API from the UniProt Consortium.
- Covers Swiss-Prot (reviewed) and TrEMBL (unreviewed) protein entries.
