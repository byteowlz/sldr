# Edits flow back along provenance; shared sources never accept silent writes

Markdown slides, templates, and flavors are the sources of truth; rendered HTML is never the master, and presenter edit mode must round-trip rather than fork. Since one rendered slide mixes nodes from all three sources, we route edits by provenance (via source-mapped HTML): slide-content nodes are editable in the presenter and `apply-comments` writes them back to the slide file; template- and flavor-origin nodes are inert in edit mode. Shared sources are edited only in deliberate contexts (text editor, satellite editors) that first show blast radius ("`two-cols` is used by 14 slides").

## Considered Options

- **Classify-and-route on apply** (rejected): let edit mode touch anything, then sort the diff into slide/template/flavor changes at apply time. Rejected because a user restructuring template markup from inside one deck has no idea they are editing every past and future slide using that template — consequence must be visible *before* the edit, not at apply time.
- **Fork-on-write** (rejected): materialize a per-slide template override when template-origin markup is edited. Rejected as template sprawl — the structural equivalent of `-v2` slide copies, destroying the reuse model. This-slide-only deviation is expressed through existing per-slide knobs (frontmatter `template`, `align`/`valign`) or by deliberately creating a new named template.

## Consequences

- The source-map format must record which source file each node came from, not just a range.
- Presenter edit mode is honestly described as "edit content in place; structure and style are read-only here, by design."
- Blast-radius reporting needs a core primitive (enumerate slides by template/flavor usage) — it must work one-door, without any satellite.
