# What sldr is not

Scope boundaries decided alongside the vision. Each of these is a plausible feature request; the no is deliberate.

1. **Not a hosting or sharing platform.** The deliverable is a self-contained file the user owns. A curation/collaboration platform may exist as a separate product, but it is not part of sldr and is bound by the satellite rules (ADR-0001).
2. **Text-first, forever.** Markdown and plain files remain the canonical authoring surface; visual editing exists only in satellites. This is the price of bitter-lesson resistance — the day editing text becomes the fallback rather than the default, the one-door principle is dead. Visual tools are welcome; they are never primary.
3. **PowerPoint does not define the authoring model.** [ADR-0010](0010-preservation-aware-presentation-interoperability.md) narrowly revises the original exit-door-only policy: the PPTX adapter may perform bounded native round trips, explicitly mapped immutable template-backed export, and opt-in inert external-package preservation with complete loss/conflict reporting. Markdown/layout/flavor/playlist factoring remains canonical. A general Office editor, unrestricted foreign-deck conversion, automatic layout guessing, and silent shared-source edits remain excluded.
4. **No realtime collaboration in the core.** Git is the synchronization and history layer. A satellite may offer live sessions, but it must converge to plain files in git and may never extend core formats to do it.
5. **No embedded intelligence.** The binary makes no model calls and ships no judgment heuristics. The agent using sldr is the only intelligence needed; sldr may shell out to a user-configured external agent (`agent` config key), but judgment always lives outside the binary.

## Supersession

Item 3 is narrowly superseded by accepted [ADR-0010](0010-preservation-aware-presentation-interoperability.md). All other boundaries remain unchanged. Acceptance is scope permission, not evidence of shipped fidelity; consult the [capability contract](../presentation-interoperability.md).
