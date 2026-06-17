# What sldr is not

Scope boundaries decided alongside the vision. Each of these is a plausible feature request; the no is deliberate.

1. **Not a hosting or sharing platform.** The deliverable is a self-contained file the user owns. A curation/collaboration platform may exist as a separate product, but it is not part of sldr and is bound by the satellite rules (ADR-0001).
2. **Text-first, forever.** Markdown and plain files remain the canonical authoring surface; visual editing exists only in satellites. This is the price of bitter-lesson resistance — the day editing text becomes the fallback rather than the default, the one-door principle is dead. Visual tools are welcome; they are never primary.
3. **PPTX is an exit door, never a format target.** Export is lossy by design and fidelity is not promised. The core will never contain a PPTX importer or let PowerPoint's model shape sldr formats. If PPTX import ever happens, it is agent work — a capable model translating decks into canonical slides/layouts/flavors — producing plain files, not a core parser.
4. **No realtime collaboration in the core.** Git is the synchronization and history layer. A satellite may offer live sessions, but it must converge to plain files in git and may never extend core formats to do it.
5. **No embedded intelligence.** The binary makes no model calls and ships no judgment heuristics. The agent using sldr is the only intelligence needed; sldr may shell out to a user-configured external agent (`agent` config key), but judgment always lives outside the binary.
