# The content/layout/flavor factoring is the product, and both style axes are open data

We questioned whether the content/layout/flavor separation survives the bitter lesson — strong models can emit bespoke, art-directed HTML per slide, so why factor at all? We concluded the factoring is not hand-coded intelligence (which better models obsolete) but data normalization (which better models exploit): it is the mechanism behind everything sldr promises — one-command restyle requires style factored out, curation and reuse require content not entangled with its look, determinism requires compilation rather than regeneration (regeneration is not idempotent; it can alter your own numbers). Without the factoring, sldr is a generic markdown-to-deck converter that a strong agent doesn't need.

Uniqueness of output therefore comes from keeping both style axes open as data, not from bypassing the factoring:

- **Layouts become data.** Today layouts are hardcoded match arms in `wrap_slide` with their CSS welded into `base.css` — a closed set that caps expressiveness at compile time. A layout must become a user-authorable file (HTML structure + scoped CSS) so that a new look is cheap agent work, named and reusable. Expressiveness then scales with model capability instead of fighting it.
- **Flavors stay two-tier.** Named tokens are the binding contract (layouts consume only `var(--sldr-*)` tokens; flavors define them — this is what keeps arbitrary flavor × layout combinations and one-command restyle working). `custom_css` is the unbounded escape hatch for the long tail. The schema grows by promotion: when a `custom_css` pattern recurs across flavors, it becomes a token. Never speculatively.

A look that cannot be factored into (content, layout, flavor) signals the format vocabulary needs widening — never that the factoring should be bypassed.

## Consequences

- Data-driven layouts are the most important structural task in the repo; the flavor axis already got this treatment (extended flavor schema), the layout axis did not.
- Layout CSS referencing flavor internals or flavor `custom_css` referencing layout markup details is coupling debt — tolerated in the escape hatch, banned in the token tier.
- Per-slide uniqueness is expressed as "author a new layout + flavor," never as bespoke HTML in a slide.
