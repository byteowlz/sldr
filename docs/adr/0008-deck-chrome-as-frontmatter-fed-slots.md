# Deck chrome is frontmatter/flavor-fed layout slots, not a new rendering layer

Branded decks need persistent framing — a headline/subheadline zone, a footer line, logos, a permanent background, and (for web-clipping slides) a source-attribution line — consistent across many slides. The question was where this "chrome" lives without breaking the content/layout/flavor factoring.

We resolved it by **splitting chrome along the existing factoring** rather than inventing a new master-slide concept:

- **Style chrome → flavor.** The permanent background (`background` type=image/svg, already full-bleed) and logos (`logos` with per-layout placement, already supported) are style — they belong to the flavor and swap with it. Added one field: `footer` (the deck's default copyright/footnote line).
- **Content chrome → frontmatter.** Headline (`title`), subheadline (`subtitle`), web-clipping attribution (`source` + optional `source_url`), and a per-slide `footer` override are slide content — they belong in frontmatter.
- **Placement → layout, via slots.** The engine feeds these into four new layout slots — `{{headline}}`, `{{subheadline}}`, `{{footer}}`, `{{source}}` — using the *exact same slot mechanism* layouts already use for `{{content}}`/`{{left}}`/etc. A "framed" layout pins them in fixed chrome positions; a plain layout ignores them; an empty slot collapses to nothing.

## Considered Options

- **A master-slide / frame layer** the engine renders around every slide (rejected): a genuinely new rendering concept and a second place structure lives, competing with layouts. It would also entangle style (logos) and content (headline) in one engine-owned blob, violating the factoring.
- **Chrome baked into each layout's markup** (rejected): repetitive, and a footer/source change would mean editing every layout.
- **Chrome as frontmatter/flavor-fed slots** (chosen): no new paradigm — chrome is just more slots, fed from frontmatter and flavor instead of the markdown split. Headline/subheadline come from frontmatter (not markdown `#`) so a framed layout can place them in a fixed zone independent of the body, matching PowerPoint's title-placeholder model.

## Consequences

- `SlideMetadata` gains `subtitle`, `source`, `source_url`, `footer`; `Flavor` gains `footer`; `wrap_slide` gains a `Chrome` struct carrying the four pre-rendered slot strings.
- Framed layouts that use the chrome slots are generic and ship with the repo; the brand-specific flavor (background, logos, colors, footer text) stays in the user's library, never committed.
- The source line is self-contained: `source_url` renders as a link that is inert until clicked (no network at present).
- A slide using a framed layout puts its headline in `title:` frontmatter, not a body `#`. Plain layouts keep using markdown headings. Both conventions coexist; the layout decides which slots it reads.
