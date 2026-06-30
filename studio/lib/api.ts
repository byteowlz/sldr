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
  flavor?: string | null;
  slides: string[];
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

// --- Endpoints ---
export const api = {
  health: () => req<{ ok: boolean; version: string }>("/health"),
  slides: () => req<{ slides: SlideSummary[] }>("/slides").then((r) => r.slides),
  playlists: () =>
    req<{ playlists: Playlist[] }>("/playlists").then((r) => r.playlists),
  createPlaylist: (p: Playlist) =>
    req<Playlist>("/playlists", { method: "POST", body: JSON.stringify(p) }),
  flavors: () =>
    req<{ flavors: FlavorSummary[] }>("/flavors").then((r) => r.flavors),
  layouts: () =>
    req<{ layouts: LayoutSummary[] }>("/layouts").then((r) => r.layouts),
  layout: (name: string) => req<LayoutDetail>(`/layouts/${name}`),
  build: (playlist: string, flavor?: string) =>
    req<{ name: string; output_dir: string; html_path: string }>("/build", {
      method: "POST",
      body: JSON.stringify({ playlist, flavor }),
    }),
};
