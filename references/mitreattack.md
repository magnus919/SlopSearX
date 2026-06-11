# MITRE ATT&CK

Search MITRE ATT&CK framework for adversary tactics, techniques, and procedures. No auth required.

- **File:** `engines/mitreattack.py`
- **Type:** API
- **Auth:** None
- **Categories:** security, reference
- **Rate limit:** None published
- **Base URL:** `https://raw.githubusercontent.com/mitre/cti/master/enterprise-attack/enterprise-attack.json`

## Usage

```
?q=<query>&engines=mitreattack
?q=<query>&categories=reference
```

## Response

Returns ATT&CK technique results with technique ID, name, tactic, platform, detection guidance, and mitigation references.

## Notes

- Free, public data from MITRE.
- Based on the STIX/TAXII representation of the ATT&CK framework.
- Covers enterprise, mobile, and ICS tactics and techniques.
