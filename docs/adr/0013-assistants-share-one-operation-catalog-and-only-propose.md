---
status: proposed
---

# Assistants share one operation catalog, and agents only propose

**Status:** proposed, 2026-10-21. Not accepted until the owner confirms. Nothing below
is implemented.

## Context

The owner wants a Studio view that feels like classic PowerPoint so people can switch
easily, with document tabs, the existing column assembly, and assistance for three
audiences: people who do not use coding agents (a built-in assistant), people who
live in ChatGPT, and power users (the CLI and their own agents). The visual proposal
is the exploratory Design Studio probe `sldr-classic-editor-r1`
(`/sldr/classic-editor-r1/`); it is not selected.

Constraints that shape the answer:

- [ADR-0004](0004-what-sldr-is-not.md) §5: the binary makes no model calls and never
  invokes an agent. Agents call sldr, never the reverse. §2: canonical authoring stays
  text; visual editing lives in satellites.
- [ADR-0009](0009-sldr-studio-api-first-web-satellite.md): the HTTP API is the product;
  Studio is a satellite that writes only canonical files. Its bearer token is not
  hardened for public exposure.
- [ADR-0011](0011-zone-document-is-the-editor-and-pptx-model.md): edits route through
  the zone document's bindings and provenance; shared slides are never edited silently.
- WebMCP (W3C Community Group draft; `document.modelContext.registerTool`) needs a
  secure context. ChatGPT's desktop built-in browser supports a subset: JavaScript
  registration on the top-level page only, no iframes, no declarative forms; usage
  depends on model and workspace policy. A remote MCP server is ChatGPT's other route
  and needs a public HTTPS endpoint or a tunnel.
- pi-durable (Earendil, experimental API) gives durable conversations, tools and
  replay over SQLite/JSONL; a sibling byteowlz prototype already runs it as a host
  process whose tools create pending changes the user keeps or reverts. oqto-desktop drives a normal `pi` and
  never embeds one (its ADR-0001).

## Decision (proposed)

1. **One operation catalog.** Agent-facing operations are core/API functions with
   JSON schemas generated from the Rust types: *read* (list presentations, read one,
   current view, find slides, where-used, zone document), *navigate* (open, go to),
   and *propose* (insert slide references, zone text, layout; withdraw own proposal).
   Every door is a thin adapter over the same catalog: Studio buttons, the built-in
   assistant, WebMCP site tools, the CLI, and later a remote MCP server. No door
   defines its own tools.
2. **Agents propose; people keep.** A proposal is a pending change held by
   `sldr-server` outside the library: target binding (zone/byte range or playlist
   line), base content hash, proposed content, actor, reason. *Keep* applies it
   through the normal provenance-routed write and refuses on a base-hash mismatch;
   *Revert* discards it. Keep and revert are human-only and are never exposed as agent
   tools. A shared slide's proposal asks where it applies (everywhere, or a
   presentation-specific file), as for a human edit. Proposals are not a canonical
   format; deleting them loses no library data. CLI users and their own agents keep
   writing files directly, as today.
3. **The built-in assistant is a separate satellite.** An optional `sldr-assistant`
   process (pi-durable on Bun/Node, SQLite under the XDG state directory) that the
   user configures and starts explicitly. It calls `sldr-server` as an ordinary API
   client with the catalog as its tools, records every call in a trace, and sends
   nothing to a model before the user sends a message. `sldr` and `sldr-server` never
   start it, embed it or call a model; the Rust core and deck artifacts stay node-free.
4. **WebMCP is progressive enhancement of the standalone Studio.** The top-level Studio
   page registers the catalog as `sldr_*` site tools when `document.modelContext`
   exists, with `readOnlyHint` on reads, `untrustedContentHint` where results carry
   slide text, and `consequentialHint: false` for proposals because nothing changes
   until a person keeps it. It requires HTTPS. Inside Oqto the Studio runs in an app
   frame where ChatGPT does not discover tools; there, Oqto's own Pi with sldr's skill
   is the assistant.
5. **Remote MCP comes later.** A remote MCP server (ChatGPT plugin, other clients)
   reuses the catalog but waits for real authentication suitable for public exposure.

## Considered options

- **Agent loop inside sldr-server** (rejected): violates ADR-0004 §5 and puts model
  credentials in the core service.
- **Agents write files directly through the Studio** (rejected for this audience):
  people new to assistants need to see and approve each change; power users keep
  direct writes through the CLI.
- **Separate tool definitions per door** (rejected): drift between what ChatGPT, the
  assistant and the CLI can do.
- **WebMCP only** (rejected as the whole answer): needs an open page, ChatGPT desktop
  and HTTPS; does not serve people without ChatGPT.
- **Remote MCP first** (deferred): public exposure needs auth sldr does not have yet.

## Consequences

- `sldr-server` gains a proposal store and keep/revert endpoints with base-hash checks;
  the zone document supplies bindings. The ADR-0011 primitives come first.
- The catalog's schemas are generated, versioned and tested once; adapters get
  conformance tests (same inputs, same proposals) rather than separate logic.
- A classic Edit view becomes a Studio surface beside the Board. It is a satellite view
  over canonical files, not a change to the authoring model of ADR-0004 §2; its visual
  design waits for the owner's selection in the Design Studio.
- `sldr-assistant` adds a Bun/Node dependency to that satellite only, and a
  pinned, experimental pi-durable version whose API may change between releases.
- WebMCP is a W3C Community Group draft whose API surface has already moved
  (`navigator.modelContext` to `document.modelContext`); the adapter must degrade
  silently when the API is absent or changes, and is verified against ChatGPT's
  browser before any claim of ChatGPT support.
