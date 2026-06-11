---
name: sldr-design
description: |
  Design-focused skill for sldr decks: polish, audit, critique, shape, craft,
  bolder, quieter, distill. Triggers: "design review my deck", "make this
  punchier", "tighten the copy", "audit the deck", "critique slides", "shape
  a presentation", "what flavor fits", "extend a flavor", "make it quieter",
  "make it bolder", "distill these slides".
---

# sldr Design Skill

This skill makes design decisions about sldr decks — picking the right flavor,
extending it without breaking it, and improving slide-level craft. It is a
companion to `sldr-presentations` (which handles the mechanics of creating
slides and playlists). Reach for this one whenever the question is taste,
voice, or visual coherence.

The mental model: sldr separates **content** (markdown slides) from **style**
(flavors) from **layout** (layouts). The design skill operates on all three,
but the design system lives in the **flavor**. When in doubt: respect the
flavor and adapt the content, not the other way around.

## The workflow (deck-level)

For any "build me a deck" or "redesign this deck" request, follow this order:

### 1. Ask about occasion + mood

Before reading anything, ask the user two questions:

> 1. **What's the occasion?** (founder pitch, research synthesis, brand
>    manifesto, classroom kickoff, etc.)
> 2. **What mood / vibe?** (confident & punchy, quiet & literary, warm &
>    playful, dark & moody, etc.)

Wait for the answer. Even when the brief seems obvious, ask — taste surprises.

### 2. Read the flavor index, pick 3 candidates

Run:

```bash
sldr ls flavors --json
```

This emits a BHT-compatible index: each flavor has `mood`, `tone`, `occasion`,
`formality`, `density`, `scheme`, `best_for`, `avoid_for`. Match the user's
stated occasion + mood against those fields. Pick **three** candidates that
are *different enough* to give a real choice — e.g. one editorial, one warmer
alternative, one wildcard that re-interprets the brief.

If a flavor has no curation block, fall back to its `description` and read its
`flavor.toml` to judge the palette/typography directly.

### 3. Preview each candidate's title slide

For each of the 3 candidates, build a single-slide preview using the user's
real title / subtitle / author / date:

```bash
sldr preview <slide-name> --flavor <candidate>
```

Make the preview real, not generic. Three side-by-side previews almost always
make the choice obvious without further discussion.

### 4. User picks → build the deck

Once the user picks, build the full deck with that flavor. If a slide needs
a layout the deck doesn't have, **design it inside the chosen flavor's
language** — see `references/extending-flavors.md`. Do not bail to a different
flavor and do not import a new visual vocabulary mid-deck.

### 5. Send the file path

Always end by giving the user the absolute path to the built HTML, on its
own line, so it's clickable in their terminal.

## Tone-first matching

Flavors carry **tones**, not industries. `best_for` describes how a flavor
*feels*, not what industry it belongs to. A confident editorial flavor can
carry a tech talk; a quiet serif flavor can carry a finance review. The
user's taste wins.

When matching:

- **Lead with `mood` + `tone` + `best_for`.** Match the *feeling* the user
  asked for.
- **Treat `avoid_for` as a soft warning.** If the user has explicitly asked
  for what `avoid_for` flags against, the user wins — but mention it.
- **Use `formality` and `density` as sanity checks.** A low-formality flavor
  for a board presentation is probably wrong regardless of tone overlap.
- **Don't over-fit on `occasion`.** That field is example contexts, not
  the canon list.
- **Ask about *tone*, not *industry*.** Good question: "polished &
  authoritative, or warm & design-led?" Bad question: "is this finance or
  tech?"

## Commands

Each command below operates on a target — usually one or more slide files,
sometimes a whole deck, sometimes a flavor. The user invokes them by name
("polish slide 3", "make slide 7 quieter", "distill this deck").

### `polish <target>`
Tighten craft without changing intent. Fix awkward line breaks, balance
headline/subhead pairs, normalize bullet rhythm, replace fillers, fix
inconsistent capitalization. Don't restructure. Don't introduce a new voice.
Output: the same slides, sharper.

### `audit <deck>`
Read every slide in the playlist. Report inconsistencies: tonal drift, mixed
density (too-dense slides next to too-sparse), title/section labels that
contradict each other, layouts used in incompatible ways, page-number
mismatches. Output: a numbered punch list — slide N: issue, suggested fix.
Don't change files unless asked.

### `critique <deck>`
A hard, honest design review from the deck's perspective. What's working?
What's weak? Where does the eye get lost? Which slide would you cut?
Output: 3–6 sentences, specific, no hedging. The point is signal, not
politeness.

### `shape <brief>`
Take a brief and propose a deck outline: section order, slide-by-slide
purpose, layout suggestions per slide, recommended flavor. Output: a list
the user can react to before any markdown gets written.

### `craft <outline>`
Take an outline (from `shape`, or the user's own) and write the slide
markdown files. Use `sldr slides create --file <json> --json` for batch
creation. Choose layouts from the available set (`sldr ls scaffolds`).
Output: created slide files + a draft playlist.

### `bolder <target>`
Amp up confidence. Stronger verbs, higher-contrast headlines, fewer hedges,
larger display sizes where the layout allows, single-statement slides
instead of bullet lists. Don't change the flavor — push the *content* into
the flavor's loudest register.

### `quieter <target>`
The inverse of `bolder`. Shorter sentences, lowercase eyebrows, more
whitespace, drop adjectives. If the flavor has a quiet register, use it; if
not, narrow what's on the slide rather than recoloring.

### `distill <target>`
Trim. Cut every slide that doesn't earn its place. Merge near-duplicates.
Replace 3 bullets with 1 sentence when the sentence is sharper. Output:
fewer slides, tighter sequence. Print before/after counts.

## Extending a flavor

If the user's deck needs a layout the chosen flavor doesn't ship — say, a
comparison table when the flavor only has cover / two-cols / quote — design
the missing slide *inside the flavor's existing language*. See
`references/extending-flavors.md` for the rules. The summary: same fonts,
same palette, same decoration vocabulary, same spacing rhythm. A new slide
between two existing slides should look like a natural extension of the
deck, not a graft.

## Common pitfalls

- **Don't skip the asking step.** Even with a detailed brief, ask about
  occasion + mood. The answer almost always sharpens the pick.
- **Don't substitute fonts.** Typography is the design system. If a font
  doesn't load, fix the import; don't swap families.
- **Don't recolor.** Even small accent shifts break a flavor's harmony.
  If you need a "warning" or "highlight" color and one isn't in the
  palette, pick the closest existing accent.
- **Don't combine layouts from different flavors.** Each flavor is a
  closed visual system. Pulling slide A from `editorial-serif` and slide B
  from `neo-grid-bold` will look amateur. Extending one flavor (§5) is
  fine; mashing two is not.
- **Don't strip "extra" decoration thinking it's noise.** Page numbers,
  hairline rules, accent shapes — they're part of the identity.
- **Don't try to "modernize" an existing flavor.** It's working as
  designed. If it feels dated for the brief, pick a different flavor.

## Output contract

For every artifact you produce — single-slide previews, intermediate
iterations, the final deck — do both:

1. **Open the file in the browser** (`open <path>` on macOS, `xdg-open`
   on Linux). Don't just announce it.
2. **Send the absolute file path**, on its own line, formatted as a path.

For the final deck, also include:
- A one-line note about which flavor you picked and *why* (the tone match).
- Any caveats — e.g., "I designed slides 4 and 7 from scratch using the
  flavor's design system since you needed a comparison table and a 4-column
  timeline that no layout covered."

Don't narrate every step. The user wants the artifact + path + one-line
rationale, not a transcript.

## Where the design system lives

| Concern | Token home | Touch via |
|---|---|---|
| Fonts | `flavor.toml [typography]` | Edit the flavor, never inline |
| Palette | `flavor.toml [colors] / [dark_colors]` | Edit the flavor |
| Spacing rhythm | `flavor.toml [spacing]` | Edit the flavor |
| Motion / easing | `flavor.toml [motion]` | Edit the flavor |
| Decoration | `flavor.toml [decoration]` | Edit the flavor |
| Code style | `flavor.toml [code]` | Edit the flavor |
| Logos | `flavor.toml [[logos]]` + `assets/` | Edit the flavor |
| Slide layout | `scaffolds/*.md` (12 bundled) | Pick via `layout:` frontmatter |
| Slide content | `slides/*.md` | Edit the slide |
| Curation hints | `flavor.toml [curation]` | Edit the flavor |

The rule of thumb: if a change should affect every slide in the deck, edit
the flavor. If it should affect just one slide, edit the slide. Never
hand-roll CSS in slide markdown — that drift is what flavors exist to
prevent.
