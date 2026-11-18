# Selecting PPTX master layouts

`sldr master template.pptx` inventories the layouts and placeholder indices of
an existing PowerPoint master. `--json` includes each layout's `selector`;
`--skeleton` lists selectors in a commented mapping template.

Layout display names are not identifiers: PowerPoint permits several layouts
named `Content` in the same slide master. A name-only mapping works only when
that trimmed name is unique within the selected `slide_master`. Ambiguous names
fail before export, listing explicit candidate selectors; the last layout is
never silently selected. The same rules apply to `fallback`.

Use the exact `part:` selector from the inventory to select a particular layout:

```toml
master = "template.pptx"
slide_master = 1
fallback = "part:ppt/slideLayouts/slideLayout2.xml"

[layouts.framed-section]
layout = "part:ppt/slideLayouts/slideLayout1.xml"
zones = { headline = 0, subheadline = 13 }
```

Selectors identify package parts, not layout positions, relationship IDs or
names. They survive layout renaming and reordering within that source package.
They must belong to the selected slide master; a missing selector fails rather
than falling back to a name. `part:` is reserved for explicit selectors. Existing
maps using unique layout names need no changes. Surrounding whitespace is ignored
for both forms.

Replacing or resaving the template may change its part paths or placeholder
indices. Inventory the new source asset and verify the map again. The original
master is never modified; exported slides reference its selected layouts and
retain the mapped placeholder indices and artwork.

Regression proof: `just test-one master::tests` covers duplicate names, explicit
selection of both candidates, their actual slide-layout relationships and
placeholder indices, strict reimport, fallback, stale selectors, and existing
unique-name maps using synthetic packages only.
