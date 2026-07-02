// Thin client over the sldr-server API (ADR-0009). Same Bearer-token pattern as
// Oqto's authFetch, so the section components port over unchanged — only the
// token storage/shell differs.

const TOKEN_KEY = "sldr:token";
export const getToken = () => localStorage.getItem(TOKEN_KEY) ?? "";
export const setToken = (t: string) => localStorage.setItem(TOKEN_KEY, t);
export const clearToken = () => localStorage.removeItem(TOKEN_KEY);

export class ApiError extends Error {
  status: number;
  constructor(status: number, message: string) {
    super(message);
    this.status = status;
  }
}

async function req<T>(path: string, init?: RequestInit): Promise<T> {
  const token = getToken();
  const res = await fetch(`/api${path}`, {
    ...init,
    headers: {
      "Content-Type": "application/json",
      ...(token ? { Authorization: `Bearer ${token}` } : {}),
      ...(init?.headers ?? {}),
    },
  });
  if (!res.ok) {
    const body = await res.text();
    throw new ApiError(res.status, body || res.statusText);
  }
  return res.status === 204 ? (undefined as T) : res.json();
}

// --- Types (mirror the server models) ---
export interface SlideSummary {
  name: string;
  relative_path: string;
  metadata: Record<string, unknown>;
}
export interface Playlist {
  name: string;
  title?: string | null;
  description?: string | null;
  flavor?: string | null;
  slides: string[];
  /// Render opts — passed through untouched so saves don't wipe them.
  render?: unknown;
}
export interface LayoutSummary {
  name: string;
  category: string | null;
  builtin: boolean;
  zone_count: number;
}
export interface Zone {
  name: string;
  ph: string | null;
  idx: number | null;
  rep: string;
  x: number;
  y: number;
  w: number;
  h: number;
}
export interface LayoutDetail extends LayoutSummary {
  source: string;
  zones: Zone[];
}
export interface FlavorSummary {
  name: string;
  display_name?: string | null;
  description?: string | null;
}

type Tokens = Record<string, string | null | undefined>;
export interface Flavor {
  name: string;
  display_name?: string | null;
  description?: string | null;
  colors: Tokens;
  dark_colors?: Tokens | null;
  typography: Tokens;
  spacing: Tokens;
  shape: Tokens;
  background: Tokens;
  footer?: string | null;
  chrome_layouts: string[];
  [k: string]: unknown;
}
export interface FlavorDetail {
  flavor: Flavor;
  css: string | null;
}

/** URL for the live sample-deck preview iframe (token in the query — iframes
 * can't set headers). */
export function samplePreviewUrl(flavor: string, bust?: number) {
  const t = encodeURIComponent(getToken());
  return `/api/preview/sample?flavor=${encodeURIComponent(flavor)}&token=${t}${bust ? `&t=${bust}` : ""}`;
}

/** URL for a single-slide preview thumbnail (auto-fits to the iframe). */
export function slidePreviewUrl(slide: string, flavor?: string) {
  const t = encodeURIComponent(getToken());
  const f = flavor ? `&flavor=${encodeURIComponent(flavor)}` : "";
  return `/api/preview/slide?slide=${encodeURIComponent(slide)}&token=${t}${f}`;
}

/** URL for a full-deck (playlist) preview — the real presenter. */
export function deckPreviewUrl(playlist: string, flavor?: string) {
  const t = encodeURIComponent(getToken());
  const f = flavor ? `&flavor=${encodeURIComponent(flavor)}` : "";
  return `/api/preview/deck?playlist=${encodeURIComponent(playlist)}&token=${t}${f}`;
}

/** URL for a layout's synthetic sample render — the zone editor's stage. */
export function layoutPreviewUrl(layout: string, flavor?: string) {
  const t = encodeURIComponent(getToken());
  const f = flavor ? `&flavor=${encodeURIComponent(flavor)}` : "";
  return `/api/preview/layout?layout=${encodeURIComponent(layout)}&token=${t}${f}`;
}

// --- Endpoints ---
export const api = {
  health: () => req<{ ok: boolean; version: string }>("/health"),
  slides: () => req<{ slides: SlideSummary[] }>("/slides").then((r) => r.slides),
  playlists: () =>
    req<{ playlists: Playlist[] }>("/playlists").then((r) => r.playlists),
  createPlaylist: (p: Playlist) =>
    req<Playlist>("/playlists", { method: "POST", body: JSON.stringify(p) }),
  updatePlaylist: (name: string, p: Playlist) =>
    req<{ name: string }>(`/playlists/${encodeURIComponent(name)}`, {
      method: "PUT",
      body: JSON.stringify(p),
    }),
  saveZones: (name: string, zones: Zone[]) =>
    req<LayoutDetail>(`/layouts/${encodeURIComponent(name)}/zones`, {
      method: "PUT",
      body: JSON.stringify({ zones }),
    }),
  flavors: () =>
    req<{ flavors: FlavorSummary[] }>("/flavors").then((r) => r.flavors),
  getFlavor: (name: string) => req<FlavorDetail>(`/flavors/${name}`),
  saveFlavor: (name: string, flavor: Flavor, css: string | null) =>
    req<FlavorDetail>(`/flavors/${name}`, {
      method: "PUT",
      body: JSON.stringify({ flavor, css }),
    }),
  layouts: () =>
    req<{ layouts: LayoutSummary[] }>("/layouts").then((r) => r.layouts),
  layout: (name: string) => req<LayoutDetail>(`/layouts/${name}`),
  build: (playlist: string, flavor?: string) =>
    req<{ name: string; output_dir: string; html_path: string }>("/build", {
      method: "POST",
      body: JSON.stringify({ playlist, flavor }),
    }),
};
