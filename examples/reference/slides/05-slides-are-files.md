---
title: Slides are files
description: Default layout — frontmatter and markdown body
tags: [reference, guide]
layout: default
---

# A slide is a markdown file

Every slide starts with YAML frontmatter, then ordinary markdown:

```yaml
---
title: My slide
layout: two-cols     # pick any layout by name
tags: [demo]
---
```

The `layout:` field is the only structural decision you make per slide. Everything else — colors, fonts, spacing — comes from the flavor at build time.
