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

## Layout

- `src/lib/api.ts` — typed client over the sldr API (Bearer token, same pattern
  as Oqto's `authFetch`; the sections port unchanged).
- `src/sections/*` — Decks (build), Layouts (source + zones), Flavors. The
  Oqto-portable pieces.
- `src/App.tsx` — the standalone-only shell (token gate, nav, theme). Oqto would
  replace this with its own shell and register the sections as an app.
