# Freeform blocks are the escape hatch, not the road

**Status:** accepted, 2026-10-01.

## Context

Real slide decks contain compositions no layout anticipates: a diagram with
labels at specific spots, a figure beside three annotations, a one-off
poster. Measured against real decks (the parity harness, `tools/parity`),
an agent that cannot place things ends up writing raw HTML with absolute
positions inside the markdown. That slide looks right and is dead: no
flavor tokens, no layout, nothing PowerPoint can edit, nothing the next
slide can reuse. If that became the normal way to use sldr, there would be
no reason for sldr to exist over plain HTML/CSS.

## Decision

1. **Blocks exist.** A body of `::block x= y= w= h=::` markers (percent of
   the slide), each followed by markdown, renders each block at its box. Two
   layouts place them: `freeform` (bare) and `framed-freeform` (deck
   chrome). Each block is still markdown: headings, lists, emphasis, one
   image. The flavor still styles it; the deck's logos and background still
   apply.

2. **Blocks are zones.** On a freeform layout the slide's blocks *are* its
   PPTX body zones, derived per slide: a text block exports as an editable
   text box at its box, a single-image block as a picture. `sldr zones`
   lists them (`block1`…), and `import --apply` writes an edited block back
   by its byte range. The per-layout zone contract (ADR-0011) gains one
   per-slide case; nothing else changes.

3. **They are the last resort.** Layouts that express structure (cards,
   flow, timeline, rows, image+text) stay the normal way. Guidance to agents
   and people: reach for blocks only when no layout fits, keep the blocks
   in markdown (never raw HTML), and when a composition recurs, promote it
   to a layout. The parity harness scores "sldr-nativeness" so an agent that
   leans on blocks or HTML is measured, not rewarded.

## Consequences

- One more thing to learn, with a clear rule for when to use it.
- A block's geometry is content (it lives in the slide), so blocks do not
  restyle across flavors the way layouts do. Acceptable for a one-off;
  the reason recurring compositions must become layouts.
- Raw HTML in markdown remains possible, as before, and remains the thing
  to avoid.
