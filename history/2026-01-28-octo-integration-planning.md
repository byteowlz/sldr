# sldr Octo Integration Planning

**Date:** 2026-01-28  
**Participants:** Tommy, Claude

## Summary

Discussed the architecture for making sldr agent-friendly and integrating it with Octo.

## Key Findings

### sldr Current State
- **sldr-core**: Working library (slides, skeletons, flavors, presentations, fuzzy matching)
- **sldr-cli**: Working CLI (build, ls, new, add, rm, search, preview, open, config, init)
- ~3300 lines of Rust code, compiles clean
- Missing: HTTP API, templates with v-drag, populated flavors

### Slidev's Built-in Visual Positioning

**Major discovery:** Slidev natively supports draggable elements with automatic source file updates via `v-drag`:

```markdown
---
dragPos:
  logo: 700,30,100,_,0
  box1: 100,200,300,_,0
---

<img v-drag="'logo'" src="/logo.png">

<v-drag pos="box1">
  Drag me!
</v-drag>
```

**How it works:**
1. Double-click element to enter drag mode
2. Drag to reposition
3. Click outside to confirm
4. Slidev automatically updates `dragPos` in frontmatter

**Tested and confirmed working** - created test presentation, dragged elements, verified file was updated with new positions.

### Architecture Decision: Composable Integration

Decided against standalone sldr web UI. Instead:

| Layer | Responsibility |
|-------|----------------|
| **sldr-core** | Rust library: business logic, file ops |
| **sldr-cli** | Standalone terminal usage |
| **sldr-server** | Headless HTTP API (JSON in/out, no UI) |
| **Octo frontend** | The actual UI using Octo's design system |

**Rationale:** Embedded sldr must have Octo's look and feel, not look like a foreign iframe.

### Visual Template Editing Workflow

```
1. sldr template edit <name>
   └── Opens template in slidev with v-drag elements
   
2. Human drags elements into position
   └── Slidev auto-saves dragPos to working copy
   
3. File watcher detects changes
   └── sldr syncs dragPos back to template source
   
4. Template now has persistent positions
   └── All slides using this template inherit positions
```

Only slidev itself needs to be in an iframe (it's a Vite app). Everything else is native Octo UI components.

## What's Missing

### P1 (Backend Foundation)
1. **sldr-server crate** - HTTP API exposing sldr-core
2. **Slidev process manager** - spawn/track/kill instances, file watcher for dragPos sync
3. **Octo backend integration** - mount sldr routes at `/api/sldr/*`

### P2 (Content & UI)
4. **Templates with v-drag anchors** - title-slide, two-column, image-left/right, code-demo, quote
5. **Populated flavors** - IEM, byteowlz, it's_OWL, Leistungszentrum
6. **Octo frontend components** - SlideLibrary, SkeletonBuilder (drag-drop), FlavorPicker, PreviewPane, TemplateEditor

## Created Issues

Epic: **trx-3f4w** - sldr Octo Integration

| ID | Priority | Task |
|----|----------|------|
| trx-3f4w.1 | P1 | sldr-server crate with HTTP API |
| trx-3f4w.2 | P1 | Slidev process manager |
| trx-3f4w.5 | P1 | Octo backend: mount sldr routes |
| trx-3f4w.3 | P2 | Templates with v-drag anchors |
| trx-3f4w.4 | P2 | Populate flavor definitions |
| trx-3f4w.6 | P2 | Octo frontend: SlideLibrary component |
| trx-3f4w.7 | P2 | Octo frontend: SkeletonBuilder component |
| trx-3f4w.8 | P2 | Octo frontend: PreviewPane component |
| trx-3f4w.9 | P2 | Octo frontend: TemplateEditor component |

## Test Artifacts

Created test presentation at `~/sldr/presentations/drag-test/` demonstrating v-drag functionality.

## Next Steps

1. Set up git remote for sldr repo
2. Start implementing sldr-server (trx-3f4w.1)
3. Build slidev process manager (trx-3f4w.2)
4. Create v-drag templates (trx-3f4w.3)
