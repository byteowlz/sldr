# Extending a flavor

Sometimes a deck needs a layout the chosen flavor doesn't ship — a
comparison table when the flavor only has cover / two-cols / quote, or a
6-column timeline when the flavor only has stat cards. **Design the missing
slide inside the flavor's existing language.** Do not bail to a different
flavor. Do not import a new visual vocabulary mid-deck.

These rules are adapted from [beautiful-html-templates](https://github.com/zarazhangrui/beautiful-html-templates)'
AGENTS.md §5, translated to sldr's flavor / template / slide model. Credit
goes to that project — see `CREDITS.md` at the repo root.

## The rules

### 1. Same fonts

Use the same `font-family` declarations the flavor declares in
`[typography]` for headings / body / mono. Same weights, same letter
spacing, same line heights. Pull them in via `var(--font-heading)` /
`var(--font-body)` / `var(--font-mono)` rather than re-stating the family
in inline CSS.

If a layout needs a fourth role (eyebrow caption, footnote), reuse one of
the three existing roles at a different size — don't add a fourth font.

### 2. Same palette

Use the existing `[colors]` and `[dark_colors]` CSS variables. Concretely:
`var(--color-primary)`, `var(--color-accent)`, `var(--color-background)`,
`var(--color-text)`, `var(--color-surface)`, `var(--color-border)`.

If the new layout needs a "warning" or "highlight" color and the flavor
doesn't expose one, **pick the closest existing accent** rather than
introducing a new hex. The flavor's harmony depends on its restricted
palette; one new color breaks it.

If the flavor uses OKLCH-derived neutrals (newer flavors do via
`color-mix()`), keep using `color-mix()` for derivations rather than
hard-coding new mixes.

### 3. Same decoration vocabulary

Whatever the flavor declares in `[decoration]` — page numbers, hairline
rules, corner brackets, atmospheric radial glow, paper grain — your new
slide uses the same vocabulary. A bare slide with no decoration in a
flavor full of ornament will look broken. Conversely, a flavor with
`[decoration].intensity = 0.0` (flat-paper, like `editorial-serif`)
should *not* gain decoration on a new layout — match the restraint.

### 4. Same spacing rhythm

Use the flavor's `[spacing]` tokens: `slide_padding_x`, `slide_padding_y`,
`stack_gap`, `content_max_width`. If the new layout needs sub-spacing for
a grid, derive from `stack_gap` (`calc(var(--stack-gap) / 2)` etc.) rather
than picking a new px value out of the air.

The 4pt grid still applies. New padding values should be multiples of 4
(or `clamp()`s of multiples).

### 5. Same component grammar

Every flavor builds slides from a small set of reusable components — stat
cards, quote blocks, two-column layouts, etc. — each with a specific
internal structure (e.g. *large number → label → description → mono
caption*). When you need a new component, **reuse an existing component's
internal structure** rather than inventing a different one.

Look at how the flavor's `templates/` slides organize content. Match that
hierarchy: same heading level for the headline, same wrapper for the
supporting copy, same caption treatment.

### 6. Same chrome

If the rest of the deck shows a top section label, a bottom page number, a
corner mark — your new slide shows them too, in the same place, in the
same style. Page numbers in particular: keep the `NN / TT` convention if
that's what the deck uses.

### 7. Same motion

The flavor's `[motion]` tokens (`transition`, `easing`, `duration`) drive
slide-to-slide and in-slide animations. New components animate with the
same easing curves and durations. If the flavor declares
`prefers-reduced-motion` overrides, your new component respects them too.

### 8. Same code style

If the slide includes a code block, use the flavor's `[code].syntax_theme`
and `[code].frame_style`. Don't override syntect themes per slide.

## How to verify

A good test: open your new slide between two existing slides in the deck.

If it visibly **belongs** — same fonts, same colors, same decorations,
same spacing rhythm — you've succeeded.

If it looks like a different flavor grafted on, you've failed. Common
tells: a font that's slightly off; an accent that wasn't already in the
palette; padding that doesn't match; a missing page number; a decoration
density that's higher or lower than the rest of the deck.

Redo, don't ship.

## When the rule conflicts with the user's ask

If the user explicitly asks for something the flavor can't do — "make the
title slide use Helvetica" when the flavor is set to `editorial-serif` —
the answer isn't "graft Helvetica into one slide". The answer is:

1. Tell the user what the flavor is committed to and why (one sentence).
2. Offer two paths:
   - **Stay in flavor:** here's how I'd interpret your ask within the
     existing vocabulary (e.g. "I can use the flavor's `body_font` at
     display size — it's the closest match").
   - **Switch flavor:** if you really want Helvetica, the closest flavor
     in the library is `<x>` — should I rebuild the deck in that?
3. Wait for the pick.

Mashing two flavors together is the one move that's never the right
answer.

## Quick checklist for a new layout inside an existing flavor

Before declaring the layout done:

- [ ] All fonts come from `var(--font-*)` declared by the flavor
- [ ] All colors come from `var(--color-*)` declared by the flavor
- [ ] All padding/spacing is a multiple of 4 (or `clamp()` of multiples)
- [ ] Decoration density matches the rest of the deck
- [ ] Page-number / chrome conventions match
- [ ] Motion / easing matches the flavor's `[motion]`
- [ ] Code blocks (if any) use the flavor's syntax theme
- [ ] No inline `style=""` attributes that re-state design tokens
- [ ] The slide opens in the browser cleanly between two existing slides
      and the eye doesn't catch on the seam
