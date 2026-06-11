# Oyez (SCOTUS)

Search US Supreme Court case information, oral arguments, and opinions. No auth required.

- **File:** `engines/oyez.py`
- **Type:** API
- **Auth:** None
- **Categories:** general, reference, legal
- **Rate limit:** None published
- **Base URL:** `https://api.oyez.org/cases`

## Usage

```
?q=<query>&engines=oyez
?q=<query>&categories=legal
```

## Response

Returns Supreme Court case results with case name, docket number, term, decision date, and URL to the oral argument page.

## Notes

- Free, public API from the Oyez Project at Chicago-Kent College of Law.
- Covers all Supreme Court cases with synchronized audio of oral arguments.
- Case metadata includes majority and dissenting justice information.
