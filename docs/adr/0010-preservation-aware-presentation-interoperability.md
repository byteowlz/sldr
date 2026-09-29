---
status: accepted
---

# Preservation-aware presentation interoperability

sldr should earn its place through reusable authored material and dependable transformations, not by competing with increasingly capable agents at composing one-off slides. We will strengthen the existing PPTX integration into a preservation-aware interchange layer: convert the supported subset into editable content, retain unsupported material when explicitly requested, and report every fidelity limitation. Markdown, layouts, flavors and playlists remain the canonical authoring model; PowerPoint does not become sldr's core model.

## Status and relationship to earlier decisions

This ADR is **accepted as scope policy**, not a claim that every capability is implemented. The maintainer explicitly approved immutable template-backed export and opt-in inert external-package preservation in the implementation session for `trx-4s9s.15` (2026-09-10), after being asked about that exact boundary. This narrowly supersedes item 3 of [ADR-0004](0004-what-sldr-is-not.md). Unrestricted foreign-deck conversion, automatic layout selection, a general Office editor and changes to canonical slide/layout/flavor formats remain out of scope.

Portable provenance manifests are **source-asset metadata owned by the PPTX adapter**, not new canonical slide fields or privileged editor state. They must be readable without a UI, contain relative asset references, and travel with their immutable assets. Any canonical schema extension still requires separate review. See the [tested capability contract](../presentation-interoperability.md) for shipped versus pending behavior.

[ADR-0001](0001-satellites-never-extend-core-formats.md), [ADR-0002](0002-edits-flow-back-along-provenance.md), [ADR-0003](0003-the-factoring-is-the-product.md), [ADR-0006](0006-artifacts-and-the-source-artifact-boundary.md), and [ADR-0008](0008-deck-chrome-as-frontmatter-fed-slots.md) still apply. The proposed interoperability work must not introduce privileged editor state, silently change shared layouts/flavors, or turn ordinary slides into OOXML documents. Any necessary canonical-format extension requires its own explicit review; it cannot arrive incidentally through the importer.

## Evidence motivating the proposal

A local review used installed sldr 0.8.0 and inspected source at commit `9648226`. These are separate observations; reproduce against the current checkout before attributing a runtime defect to that commit.

- An externally authored editable deck was rejected, consistently with the present import scope.
- Native export of a two-slide flavor-backed presentation succeeded but produced no media or notes-slide parts, despite five configured logos and speaker notes in the input.
- Adding a text box and notes to a copy of that export, then importing both copies, produced identical Markdown without a loss warning. The importer reported success while discarding the additions.
- PDF export rendered first-page text smaller than the same text roles on subsequent pages. A local print-unit override corrected it; that workaround is not an upstream fix.

The source reflects the bounded implementation: `sldr-pptx` accepts text, Markdown and pictures; import reconstructs known named zones and checks an application-name marker. Existing issue descriptions promise broader fidelity than these paths currently deliver. These findings justify trust and reporting fixes independently of accepting expanded import scope.

Real decks, logos and organizational details are not public test fixtures. Reproduce with synthetic, redistributable assets.

## Accepted contract

The terms below are interoperability vocabulary, not replacements for the glossary in `CONTEXT.md`.

| Disposition | What the caller may rely on |
| --- | --- |
| **Converted** | Content is represented by supported editable target elements. The report separately states visual/style limitations; editability alone does not imply visual equivalence. |
| **Preserved** | Original unsupported material and its required relationships are retained as source assets. It is not promised to be semantically editable or restylable in sldr. |
| **Baked** | A specified region becomes a picture. Its surrounding supported content remains editable; source/provenance is retained where available. |
| **Unsupported / conflicting** | A requested transformation cannot proceed without loss or an ambiguous overwrite. Strict operation fails before publishing output. |

Every input object has an accounted-for disposition. Human-readable and machine-readable reports identify the affected slide/step, element or region, reason, chosen representation, and available remedy. A successful command must not silently imply full fidelity. Missing assets, malformed relationships, unsupported geometry and font substitutions are not equivalent conditions and must be reported distinctly.

Strict operation is the recommended default for new preservation-aware workflows. Any compatibility transition for existing commands must be explicit. A reviewed lossy policy may authorize baking or dropping specific material, but a plain success message is not authorization. Never leave partial imports in the user's library after a failed strict check.

### Keep judgment outside the mechanism

Agents or humans choose the story, visual composition, slide reuse, and whether approximation is acceptable. sldr supplies inventories, capability checks, deterministic transforms and validation. Do not build a heuristic layout classifier or a closed catalog of diagram meanings into the core as a prerequisite for import.

A bounded graphical representation may be derived internally from existing authored structure, zone declarations and SVG or another proven escape hatch. Support actual repeated needs first: styled text, positioned images, boxes, lines/connectors, groups and paths. Do not introduce a universal scene graph or another public authoring language speculatively. Unsupported filters or effects may bake per region; the presence of one effect must not flatten the entire slide.

### Distinguish two export paths

1. **Flavor-derived native export** projects declared flavor/layout information into new PPTX parts. It must account for logos, backgrounds, text roles, sources, notes and geometry. Custom CSS outside the supported projection must be reported rather than silently ignored.
2. **Template-backed export** starts from an immutable user-supplied PPTX master package and an explicit mapping to selected layouts/placeholders. Preserve the master/theme/layout relationship closure instead of redrawing its branding. Do not infer the mapping silently or imply every PowerPoint feature is editable in sldr. Record the source asset hash and any necessary identifier remapping; do not modify the supplied template in place.

A PPTX master package is an external source asset, not a new sldr chrome layer. As with other irreducible media, preserving it does not imply that a flavor switch can restyle it. Report any preserved regions that do not follow the selected flavor; honor the chosen strict or preservation policy explicitly.

### Import along provenance, without silent writes

For sldr-authored decks, use versioned identifiers and mappings independent of display names and the mutable `<Application>` field. Validate their consistency against the actual package. Record supported content baselines so content edits can be distinguished from structure/style changes. Reordering and duplication must not accidentally address or overwrite the wrong source slide.

Preservation-oriented external import should begin with an inventory of actual shapes, transforms, text runs, images, groups and relationships. Convert only supported or explicitly mapped content. Retain unknown material with a rendered preview and portable provenance records rather than pretending that OCR or a screenshot is equivalent to editable content.

Preserved material belongs in user-owned source assets with a readable, versioned manifest, not an opaque editor cache or an absolute path outside the deliverable. Keep required assets in portable bundles when that mode is selected. Original parts can be restored only when their content, dependency closure and insertion context remain valid. Detect stale hashes, changed parents, overlapping edits and ambiguous identifiers; require reconciliation rather than splicing old XML over a new composition.

Shared source edits must follow ADR-0002: show their blast radius and require an explicit operation. Never route an imported style change into a library-wide flavor implicitly. Essential information must remain accessible from plain files without an interactive editor.

### Treat artifacts and source packages as untrusted inputs

Do not execute macros, OLE objects or embedded code, or fetch external relationships while inspecting a package. Bound decompression, XML parsing and media sizes; reject traversal paths and malformed relationship graphs. Distinguish inert preservation of an external link from permission to dereference it. Surface risky embedded content before handoff. Do not redistribute proprietary fonts merely to make visual tests pass.

## Alternatives considered

- **Keep screenshot export as the main bridge.** Simple and often visually adequate, but unsuitable when recipients need editable diagrams, notes and continued reuse. Retain it as an explicitly lossy fallback.
- **Let an agent rebuild every deck.** Excellent for choosing and authoring new content, but repeated generation is not preservation. It cannot substitute for a verified round-trip contract or reliable source ownership.
- **Implement a complete Office editor/model.** Rejected: excessive scope, locks sldr to OOXML and undermines the factoring. Preserve what is not understood rather than claiming to understand it.
- **Convert every foreign slide to the nearest layout.** Rejected as a default: it silently changes composition and embeds judgment in the importer. Expose evidence and mapping choices to the operator instead.

## Verification and rollout

Ship trust fixes first: loss reporting, notes/assets, robust identifiers and safe writes. The scope revision is approved; template-backed export and preservation-oriented external import still require implementation and their own acceptance evidence. Extend graphics and region baking in bounded increments. Reuse the open PPTX epic `trx-4s9s` and region-bake issue `trx-v8td`; historical closed phases remain historical rather than being represented as current guarantees.

### Tracked implementation slices

| Issue | Priority | Scope |
| --- | --- | --- |
| `trx-4s9s.6` | P1 | Element accounting, loss reports and transactional strict interchange |
| `trx-4s9s.7` | P1 | Versioned identities and conflict-aware source provenance |
| `trx-4s9s.8` | P1 | Supported branding in flavor-derived native export |
| `trx-4s9s.9` | P1 | Speaker notes, source links and supported rich content |
| `trx-4s9s.10` | P2 | Bounded editable graphical content |
| `trx-4s9s.11` | P2 | Immutable template-backed export and master inventory |
| `trx-4s9s.12` | P2 | Portable preservation-oriented import and safe re-export |
| `trx-4s9s.13` | P1 | Synthetic artifact and real-editor validation suite |
| `trx-4s9s.14` | P1 | Reproduce and resolve stale PDF print units |
| `trx-4s9s.15` | P1 | Explicit ADR scope decision and truthful capability documentation |
| `trx-v8td` | P2 | Existing per-region baking follow-up, expanded rather than duplicated |

`trx-4s9s.11` and `trx-4s9s.12` depend on the scope decision in `trx-4s9s.15`. Trust fixes, the PDF regression and validation infrastructure can begin without accepting the expanded import scope. The tracker carries the detailed dependency graph and proof instructions.

### Acceptance evidence

A synthetic five-slide acceptance deck should exercise a reusable branded master, German text, images, a branching diagram, grouped/freeform geometry, source links, notes and an unsupported visual effect. Required proof combines:

- semantic assertions and source-ownership checks through the public CLI;
- complete package/relationship validation and unique slide/notes identities;
- layout bounds, font/image fit and per-page chrome checks;
- renderer-versioned visual baselines with explicit tolerances, not universal pixel-identity claims;
- real PowerPoint open/save/edit/re-import/re-export checks, alongside automated headless rendering;
- deterministic bytes for the declared native subset; pinned engine versions and documented limits for rendered/baked output;
- negative tests proving unknown elements, missing assets, unsafe packages and conflicting edits cannot disappear under an unqualified success result.

Opening in LibreOffice alone does not prove PowerPoint compatibility. A screenshot comparison alone does not prove editability. A source round-trip without an intervening editor save does not prove the recipient workflow.

## Consequences

The integration becomes more trustworthy but also requires explicit preservation-policy, security and compatibility maintenance. Some imports will remain only partly restylable; that limitation must be visible. The core remains small and judgment-free, while the library accumulates valuable authored material and provenance rather than one-off reconstruction scripts. More capable models increase the usefulness of that substrate instead of making sldr compete with their slide-generation ability.
