# Credits

sldr stands on ideas from a number of open-source projects. This file
records the libraries, design systems, and reference works that have
shaped what sldr is today.

## Design system inspiration

### Impeccable
**Repo:** https://github.com/pbakaus/impeccable
**License:** MIT

Impeccable's design language directly informed sldr's flavor token
schema:

- OKLCH color space with `color-mix()`-derived neutrals so a single
  primary hue produces a cohesive palette
- Motion tokens (`duration-fast`/`base`/`slow`, named easings like
  `ease-out-quart` / `ease-out-expo`)
- Typography tokens (`heading-wrap: balance`, `body-wrap: pretty`,
  `font-features`, `optical-sizing`)
- `prefers-reduced-motion` accessibility defaults
- 4pt spacing grid alignment

### beautiful-html-templates
**Repo:** https://github.com/zarazhangrui/beautiful-html-templates
**License:** MIT

Zara Zhang's library of 32 hand-designed HTML slide templates is the
benchmark for what an agent-driven template library should feel like.
Concepts that map directly onto sldr's flavor model:

- Curatorial metadata per template (`mood`, `tone`, `occasion`,
  `formality`, `density`, `scheme`, `best_for`, `avoid_for`) so an
  agent can match a brief to a template by feeling, not just by
  keyword
- A single `index.json` describing the whole library so agents read
  one file instead of opening 32 folders
- An `AGENTS.md` operating manual that codifies the workflow
  (ask → match → preview → build) and the rules for extending a
  chosen template without breaking its design system
- A reusable `<deck-stage>` web component for runtime concerns
  (auto-scaling, keyboard nav, print, speaker notes, mobile tap zones)

## Core dependencies

sldr is built on a handful of Rust crates without which none of this
would exist:

- **pulldown-cmark** — CommonMark markdown parsing
  https://github.com/raphlinus/pulldown-cmark
- **syntect** — TextMate-grammar syntax highlighting
  https://github.com/trishume/syntect
- **clap** — CLI argument parsing
  https://github.com/clap-rs/clap
- **axum** + **tokio** — HTTP server and async runtime
  https://github.com/tokio-rs/axum
- **notify** — filesystem watching for live-reload
  https://github.com/notify-rs/notify
- **schemars** — JSON Schema generation from Rust types
  https://github.com/GREsau/schemars
- **fuzzy-matcher** — fuzzy slide name resolution
  https://github.com/lotabout/fuzzy-matcher
