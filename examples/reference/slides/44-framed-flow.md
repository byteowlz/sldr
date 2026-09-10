---
title: Framed Flow
subtitle: a list becomes a chain of boxes and arrows
layout: framed-flow
footer: "© sldr"
source: "Process diagrams | Reference"
tags: [reference, guide]
---
Each top-level item is one step. A leading `**bold**` line is the step title, the rest is its body; a task check (`- [x]`) highlights a step. The boxes use the flavor's diagram tokens, so no colours live in the slide.

- ![Icon](ref-square.svg)
  **Collect**
  Gather the raw material.
- **Sort**
  Order it by what matters.
- [x] **Decide**
  The step to look at right now.
- **Ship**
  Hand it over.

* Numbered steps: use an ordered list and the titles are numbered for you.
* Text after the chain sits below it as commentary. This list uses `*` markers: a second `-` list would merge into the chain.

<!-- notes: Same content in a `::left::` / `::right::` split runs the chain top to bottom on the left with commentary on the right — see the next slide. -->
