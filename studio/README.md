# sldr studio (frontend)

A standalone web UI for sldr — manage the slide library, build decks, and inspect
flavors/layouts — talking to the `sldr-server` HTTP API (ADR-0009). Built to
mirror Oqto's stack (React 19 + Vite + Tailwind 4 + React Query) so it ports into
Oqto as an app later with no restyle.

This is a **satellite**: it writes only canonical files via the API, and it is
entirely separate from the Rust workspace — `cargo build` never touches it.

## Develop

```bash
# 1. run the API (from the repo root)
SLDR_API_TOKEN=dev sldr-server          # listens on :4100

# 2. run the dev server (proxies /api -> :4100)
bun install
bun dev                                 # http://localhost:5173
```

## Build + serve standalone (the "phone via my server" deployment)

```bash
bun run build                           # -> studio/dist

# sldr-server serves the SPA at / and the API under /api on one origin:
SLDR_API_TOKEN=<token> \
SLDR_STUDIO_DIR=studio/dist \
SLDR_SERVER_ADDR=0.0.0.0:4100 \
SLDR_TLS=1 \
sldr-server
```

Then open `https://<host>:4100` over Tailscale from any device, accept the
certificate once, and enter the token.

**HTTPS is required off-localhost.** Browsers treat `localhost` as a secure
context but not a tailnet hostname over plain HTTP, and the editor relies on
APIs that need one — over HTTP it hangs. `SLDR_TLS=1` generates a self-signed
pair once (under the sldr data dir, `tls/`), covering `localhost`, the
machine's hostnames and its non-loopback IPv4s, and reuses it on every start
so the one-time acceptance sticks. Bring your own with `SLDR_TLS_CERT` and
`SLDR_TLS_KEY` (e.g. a `tailscale cert` pair) to skip the warning entirely.

## Layout (ADR-0011)

- `lib/backend/` — the one interface every section talks to, with two
  implementations: `http.ts` (sldr-server, bearer token — standalone) and
  `oqto.ts` (the Oqto app SDK: host-run operations, one id per method, previews
  returned as HTML for `srcdoc` frames). `index.tsx` picks one at startup and
  provides `useBackend()`; `preview.ts` wraps previews in react-query so
  `SlideFrame` can render a URL or HTML alike. Sections never know which
  backend they run on.
- `lib/api.ts` + `lib/api-types.ts` — the HTTP client and the TypeScript types
  generated from the Rust models (`bun run types`; never hand-edit the latter).
- `lib/docs/store.ts` — open documents (tabs): playlists, slides, layouts,
  flavors, with dirty flags. The only state the studio owns; persisted per user
  in localStorage, never a file the CLI can see.
- `src/sections/compose/` — the composer split into Browser (library rail),
  Stage (real render + source drawer), Strip (timeline), Inspector (frontmatter,
  where-used, activity) and the `useDeck` hook (reference-only deck edits).
- `src/sections/*` — Composer, Layouts, Flavors: orchestration only.
- `src/App.tsx` — the standalone shell (token gate, nav, theme). Inside Oqto the
  host authenticates, so the gate is skipped.

Rules: every question the UI asks is a core function first (CLI + API), the
studio never ranks, resolves or diffs; no secure-context-only browser APIs
without a fallback (plain HTTP on a tailnet must keep working).
