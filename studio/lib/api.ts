// Thin client over the sldr-server API (ADR-0009). Same Bearer-token pattern as
// Oqto's authFetch, so the section components port over unchanged — only the
// token storage/shell differs.

import type {
  Candidate,
  Hit,
  LayoutUsage,
  MediaIndex,
  SlideUsage,
  UsageIndex,
  ZoneDocument,
} from "./api-types";
export type * from "./api-types";

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
  /// Full flavor objects come back from /flavors — colors feed the swatches.
  colors?: Record<string, string | null | undefined>;
}

export interface SlideDetail {
  name: string;
  relative_path: string;
  metadata: Record<string, unknown>;
  content: string;
  /// Raw file source (frontmatter + body) — what the source drawer edits.
  raw: string;
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
export function slidePreviewUrl(slide: string, flavor?: string, layout?: string) {
  const t = encodeURIComponent(getToken());
  const f = flavor ? `&flavor=${encodeURIComponent(flavor)}` : "";
  const l = layout ? `&layout=${encodeURIComponent(layout)}` : "";
  return `/api/preview/slide?slide=${encodeURIComponent(slide)}&token=${t}${f}${l}`;
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
  slideDetail: (name: string) =>
    req<SlideDetail>(`/slides/${encodeURIComponent(name)}`),
  saveSlideRaw: (name: string, raw: string) =>
    req<SlideDetail>(`/slides/${encodeURIComponent(name)}`, {
      method: "PUT",
      body: JSON.stringify({ raw }),
    }),
  saveSlideMeta: (name: string, metadata: Record<string, unknown>) =>
    req<SlideDetail>(`/slides/${encodeURIComponent(name)}`, {
      method: "PUT",
      body: JSON.stringify({ metadata }),
    }),
  createSlide: (name: string, subdir?: string) =>
    req<SlideDetail>("/slides", {
      method: "POST",
      body: JSON.stringify({ name, subdir }),
    }),
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
  // --- ADR-0011 primitives: generated types, one ranking, one model ---
  /** The slide's zone document: regions, bindings, write targets. */
  zones: (slide: string, opts?: { layout?: string; flavor?: string; lang?: string }) => {
    const q = new URLSearchParams();
    if (opts?.layout) q.set("layout", opts.layout);
    if (opts?.flavor) q.set("flavor", opts.flavor);
    if (opts?.lang) q.set("lang", opts.lang);
    const qs = q.toString();
    return req<ZoneDocument>(`/slides/${encodeURIComponent(slide)}/zones${qs ? `?${qs}` : ""}`);
  },
  /** Every layout ranked by fit for the slide: hides / collapses / empty. */
  layoutCandidates: (slide: string, opts?: { lang?: string; limit?: number }) => {
    const p = new URLSearchParams();
    if (opts?.lang) p.set("lang", opts.lang);
    if (opts?.limit) p.set("limit", String(opts.limit));
    const qs = p.toString();
    return req<Candidate[]>(`/slides/${encodeURIComponent(slide)}/layout-candidates${qs ? `?${qs}` : ""}`);
  },
  /** Which playlists reference a slide, and its last git touch. */
  slideUsage: (slide: string) => req<SlideUsage>(`/slides/${encodeURIComponent(slide)}/usage`),
  /** Slide → playlists for the whole library, in one call (board badges). */
  usageIndex: () => req<UsageIndex>("/usage"),
  /** Slides using a layout — the blast radius before a geometry edit. */
  layoutUsage: (layout: string) => req<LayoutUsage>(`/layouts/${encodeURIComponent(layout)}/usage`),
  /** Ranked search over names, title, tags, topic, description, body. */
  find: (q: string, opts?: { tags?: string[]; topic?: string; limit?: number }) => {
    const p = new URLSearchParams({ q });
    if (opts?.tags?.length) p.set("tags", opts.tags.join(","));
    if (opts?.topic) p.set("topic", opts.topic);
    if (opts?.limit) p.set("limit", String(opts.limit));
    return req<Hit[]>(`/find?${p.toString()}`);
  },
  /** Every media file with the slides that reference it. */
  media: () => req<MediaIndex>("/media"),
  /** URL for a listed media file (thumbnails). */
  mediaUrl: (path: string) =>
    `/api/media/${path.split("/").map(encodeURIComponent).join("/")}?token=${encodeURIComponent(getToken())}`,
  /** Store a file beside `slide` (its media/ folder); returns the markdown reference. */
  uploadMedia: async (slide: string, file: File, opts?: { name?: string; overwrite?: boolean }) => {
    const p = new URLSearchParams({ slide, name: opts?.name ?? file.name });
    if (opts?.overwrite) p.set("overwrite", "true");
    const res = await fetch(`/api/media?${p.toString()}`, {
      method: "PUT",
      headers: { Authorization: `Bearer ${getToken()}` },
      body: file,
    });
    if (!res.ok) throw new ApiError(res.status, await res.text());
    return res.json() as Promise<{ path: string; reference: string; bytes: number }>;
  },
  /** Delete a media file; refused (409) while a slide references it unless force. */
  deleteMedia: (path: string, force = false) =>
    req<{ deleted: string }>(
      `/media/${path.split("/").map(encodeURIComponent).join("/")}${force ? "?force=true" : ""}`,
      { method: "DELETE" },
    ),
  build: (playlist: string, flavor?: string) =>
    req<{ name: string; output_dir: string; html_path: string }>("/build", {
      method: "POST",
      body: JSON.stringify({ playlist, flavor }),
    }),
};
