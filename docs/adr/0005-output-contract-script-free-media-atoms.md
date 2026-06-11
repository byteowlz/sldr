# Output contract: script-free layouts and flavors, foreign media as content atoms

The built presentation contains exactly one script: the presenter engine. Layouts and flavors are script-free and iframe-free — script-driven rendering (canvas, WebGL) has no DOM, so provenance, edit mode, and the factoring itself die inside it, and once layouts run script, content migrates into script because it's expedient. The HTML+CSS+SVG envelope (grid, clip-paths, masks, blend modes, 3D transforms, SVG filters like feTurbulence for generative texture, variable fonts, CSS animation) covers virtually all high-end slide design, so the expressiveness cost is small.

Foreign media — video, iframes — are **content atoms**: opaque rectangles a *slide* may embed, in which sldr's guarantees (restyle, provenance, editability) explicitly end. They are allowed in slide content (blast radius: one slide, chosen knowingly) and banned in layouts and flavors (a layout iframe makes every slide using it network-dependent and unstylable; a flavor iframe would be the style layer injecting content).

Data visualization is a build-time transform: data lives in or beside the slide source and compiles to static SVG styled by flavor tokens. "Live" means freshly compiled — rebuild before presenting — never runtime fetching: a deck that needs the network at presentation time fails the podium test, and that exclusion follows from self-containedness independent of the script ban.

## Consequences

- Builds **warn, never refuse**, when self-containedness is degraded (network-pointing iframes, non-bundled media): "this deck needs network at presentation time."
- If a script escape hatch is ever opened, the rule is pre-decided: script may render decoration, never content.
- Genuinely live dashboards in a talk are a browser tab, not a slide.
