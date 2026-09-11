# SlopSearX portal visual system

## Direction

The selected direction is a quiet instrument: warm text, charcoal surfaces,
thin technical lines, and a small amber/green signal palette. It should feel
like a capable search instrument rather than a dashboard or a marketing page.
The interface uses the product name and task labels directly; it does not add a
slogan or motivational copy.

The foundation's rendered screens are the first high-fidelity handoff. Future
result and exceptional-state work must preserve the same hierarchy and tokens.

## Themes

Dark is the default. Darker is a deliberate near-black mode exposed by the
toggle; it is not a light/dark system switch.

| Token | Dark | Darker | Use |
| --- | --- | --- | --- |
| `--paper` | `#191a18` | `#0b0c0b` | page background |
| `--paper-raised` | `#22231f` | `#121311` | cards and controls |
| `--paper-lift` | `#2a2b26` | `#1a1b18` | hover/focus surfaces |
| `--ink` | `#f4f0e8` | `#f8f3e9` | primary text |
| `--muted` | `#a49e91` | `#a9a398` | secondary text |
| `--quiet` | `#6e6a61` | `#716c62` | nonessential metadata |
| `--accent` | `#e5b567` | `#f0bd65` | action and focus accent |
| `--signal` | `#a7d7c5` | `#b8e4d2` | healthy/provenance signal |

Primary text and controls must meet WCAG 2.2 AA contrast (4.5:1 for normal
text, 3:1 for large text and non-text focus indicators). `--quiet` is never the
sole carrier of meaning and must not be used for required control text.

## Components and behavior

- **Search field:** one prominent field on home; compact but still full-width
  enough to edit on results. Enter submits; `/` and Ctrl/Cmd-K focus it.
- **Scope control:** a labelled disclosure on narrow screens and a visible rail
  on wide screens. It reports capability and enforcement state in text.
- **Result card:** title link, one or two lines of content, source badge(s),
  category/type, and optional date. Long titles wrap; URLs are constrained and
  safe.
- **State banner:** uses an accessible heading/live region and a distinct
  border/accent. It never relies on color alone.
- **Theme toggle:** a real button with an accessible name and state. It writes
  the explicit choice to local storage when available and falls back to the
  current document when storage is blocked.

## Layout and type

Use a single reading column with generous vertical rhythm, a maximum content
width near 1180 CSS px, and a 320 CSS px minimum. The current system stack is
intentional: it avoids a remote font dependency and keeps first paint stable.
Decorative grid and gradients are subordinate to text and disappear or reduce
under `prefers-reduced-motion`/high-contrast conditions.

## Visual acceptance checklist

- [ ] Home, success, partial, empty, all-unavailable, invalid, and rate-limited
      states are reviewed in Dark and Darker.
- [ ] Desktop 1440×900 and narrow 390×844 retain usable hierarchy.
- [ ] Keyboard focus is visible on every interactive control; focus order is
      logical; no keyboard trap exists.
- [ ] 200% zoom and reflow do not hide the query or pagination.
- [ ] Long titles, missing dates, source badges, and external links remain
      legible without overflow.
- [ ] No asset or font is loaded from a third-party host by default.
