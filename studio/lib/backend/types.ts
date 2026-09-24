// The one interface every studio section talks to (ADR-0011). Two
// implementations: `http` against sldr-server (standalone), `oqto` over the
// Oqto app SDK (files grants + host operations). Sections never know which.
//
// Rules that keep the studio thin: every method here maps to a core function
// that the CLI can also reach; nothing in the studio ranks, resolves or diffs.

import type {
  Candidate,
  Flavor,
  FlavorDetail,
  FlavorSummary,
  Hit,
  LayoutDetail,
  LayoutSummary,
  LayoutUsage,
  MediaIndex,
  Playlist,
  SlideDetail,
  SlideSummary,
  SlideUsage,
  UsageIndex,
  Zone,
  ZoneDocument,
} from "../api";

/** A rendered preview: a URL the iframe can load (standalone) or the HTML
 * itself for a `srcdoc` iframe (hosted, where the app has no network). */
export type Preview = { kind: "url"; url: string } | { kind: "html"; html: string };

export type BackendId = "http" | "oqto";

export class BackendError extends Error {
  code: "not_granted" | "unsupported" | "conflict" | "not_found" | "failed";
  constructor(code: BackendError["code"], message: string) {
    super(message);
    this.code = code;
  }
}

export interface Backend {
  readonly id: BackendId;
  /** Whether `window.open` of a preview makes sense (standalone yes, sandboxed no). */
  readonly canOpenExternal: boolean;

  // --- library (canonical files) ---
  slides(): Promise<SlideSummary[]>;
  slideDetail(name: string): Promise<SlideDetail>;
  saveSlideRaw(name: string, raw: string): Promise<SlideDetail>;
  saveSlideMeta(name: string, metadata: Record<string, unknown>): Promise<SlideDetail>;
  createSlide(name: string, subdir?: string): Promise<SlideDetail>;
  playlists(): Promise<Playlist[]>;
  createPlaylist(p: Playlist): Promise<Playlist>;
  updatePlaylist(name: string, p: Playlist): Promise<{ name: string }>;
  layouts(): Promise<LayoutSummary[]>;
  layout(name: string): Promise<LayoutDetail>;
  saveZones(name: string, zones: Zone[]): Promise<LayoutDetail>;
  flavors(): Promise<FlavorSummary[]>;
  getFlavor(name: string): Promise<FlavorDetail>;
  saveFlavor(name: string, flavor: Flavor, css: string | null): Promise<FlavorDetail>;
  build(playlist: string, flavor?: string): Promise<{ name: string; output_dir: string; html_path: string }>;

  // --- derived (ADR-0011 primitives; computed by the core, never stored) ---
  zones(slide: string, opts?: { layout?: string; flavor?: string; lang?: string }): Promise<ZoneDocument>;
  layoutCandidates(slide: string, opts?: { lang?: string; limit?: number }): Promise<Candidate[]>;
  slideUsage(slide: string): Promise<SlideUsage>;
  usageIndex(): Promise<UsageIndex>;
  layoutUsage(layout: string): Promise<LayoutUsage>;
  find(q: string, opts?: { tags?: string[]; topic?: string; limit?: number }): Promise<Hit[]>;
  media(): Promise<MediaIndex>;
  uploadMedia(slide: string, file: File, opts?: { name?: string; overwrite?: boolean }): Promise<{ path: string; reference: string; bytes: number }>;
  deleteMedia(path: string, force?: boolean): Promise<{ deleted: string }>;

  // --- previews (always the real renderer's output) ---
  previewSlide(slide: string, opts?: { flavor?: string; layout?: string; bust?: number }): Promise<Preview>;
  previewDeck(playlist: string, flavor?: string): Promise<Preview>;
  previewLayout(layout: string, flavor?: string): Promise<Preview>;
  previewSample(flavor: string, bust?: number): Promise<Preview>;
  /** URL (or data URL) for a listed media file; null when it cannot be shown. */
  mediaUrl(path: string): Promise<string | null>;
}
