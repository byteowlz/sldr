// Hosted backend: the studio mounted as an Oqto app. The iframe has no
// network and no paths — it holds opaque file refs for the library's
// collections (granted by role) and a set of host-run operations for
// everything derived. Each method maps to exactly one operation id, so the
// host side is a table, not a second implementation of sldr.
//
// Fails closed: a missing grant or operation is a `not_granted` error the
// UI can show, never a silent fallback to a different data source.

import type { JsonValue, OqtoHost, OqtoOperationResult } from "@byteowlz/oqto-app-sdk";
import type { Backend, Preview } from "./types";
import { BackendError } from "./types";

/** Operation ids the sldr host adapter exposes. One per Backend method. */
export const OQTO_OPS = {
  slides: "sldr.slides.list",
  slideDetail: "sldr.slides.get",
  saveSlideRaw: "sldr.slides.write",
  saveSlideMeta: "sldr.slides.meta",
  createSlide: "sldr.slides.create",
  playlists: "sldr.playlists.list",
  createPlaylist: "sldr.playlists.create",
  updatePlaylist: "sldr.playlists.update",
  layouts: "sldr.layouts.list",
  layout: "sldr.layouts.get",
  saveZones: "sldr.layouts.zones",
  flavors: "sldr.flavors.list",
  getFlavor: "sldr.flavors.get",
  saveFlavor: "sldr.flavors.write",
  build: "sldr.build",
  zones: "sldr.zones",
  layoutCandidates: "sldr.layouts.candidates",
  slideUsage: "sldr.usage.slide",
  usageIndex: "sldr.usage.index",
  layoutUsage: "sldr.usage.layout",
  find: "sldr.find",
  media: "sldr.media.list",
  uploadMedia: "sldr.media.upload",
  deleteMedia: "sldr.media.delete",
  previewSlide: "sldr.preview.slide",
  previewDeck: "sldr.preview.deck",
  previewLayout: "sldr.preview.layout",
  previewSample: "sldr.preview.sample",
  mediaUrl: "sldr.media.read",
} as const;

function unwrap<T>(result: OqtoOperationResult, id: string): T {
  if (result.ok) return result.output as T;
  const code = result.code === "not_found" ? "not_found" : result.code === "conflict" ? "conflict" : "failed";
  throw new BackendError(code, `${id}: ${result.message ?? result.code}`);
}

async function fileToBase64(file: File): Promise<string> {
  const buf = new Uint8Array(await file.arrayBuffer());
  let s = "";
  for (let i = 0; i < buf.length; i += 0x8000) s += String.fromCharCode(...buf.subarray(i, i + 0x8000));
  return btoa(s);
}

export function oqtoBackend(host: OqtoHost): Backend {
  const ops = host.operations;
  if (!ops) throw new BackendError("not_granted", "this Oqto mount granted no operations");

  const call = async <T,>(id: string, input?: JsonValue): Promise<T> => {
    if (host.isSuspended?.()) throw new BackendError("not_granted", "app suspended by host");
    return unwrap<T>(await ops.invoke(id, input), id);
  };
  const html = async (id: string, input: JsonValue): Promise<Preview> => ({
    kind: "html",
    html: (await call<{ html: string }>(id, input)).html,
  });

  return {
    id: "oqto",
    canOpenExternal: false,

    slides: () => call(OQTO_OPS.slides),
    slideDetail: (name) => call(OQTO_OPS.slideDetail, { name }),
    saveSlideRaw: (name, raw) => call(OQTO_OPS.saveSlideRaw, { name, raw }),
    saveSlideMeta: (name, metadata) => call(OQTO_OPS.saveSlideMeta, { name, metadata: metadata as JsonValue }),
    createSlide: (name, subdir) => call(OQTO_OPS.createSlide, { name, subdir: subdir ?? null }),
    playlists: () => call(OQTO_OPS.playlists),
    createPlaylist: (p) => call(OQTO_OPS.createPlaylist, p as unknown as JsonValue),
    updatePlaylist: (name, p) => call(OQTO_OPS.updatePlaylist, { name, playlist: p as unknown as JsonValue }),
    layouts: () => call(OQTO_OPS.layouts),
    layout: (name) => call(OQTO_OPS.layout, { name }),
    saveZones: (name, zones) => call(OQTO_OPS.saveZones, { name, zones: zones as unknown as JsonValue }),
    flavors: () => call(OQTO_OPS.flavors),
    getFlavor: (name) => call(OQTO_OPS.getFlavor, { name }),
    saveFlavor: (name, flavor, css) => call(OQTO_OPS.saveFlavor, { name, flavor: flavor as unknown as JsonValue, css }),
    build: (playlist, flavor) => call(OQTO_OPS.build, { playlist, flavor: flavor ?? null }),

    zones: (slide, opts) => call(OQTO_OPS.zones, { slide, ...opts }),
    layoutCandidates: (slide, opts) => call(OQTO_OPS.layoutCandidates, { slide, ...opts }),
    slideUsage: (slide) => call(OQTO_OPS.slideUsage, { slide }),
    usageIndex: () => call(OQTO_OPS.usageIndex),
    layoutUsage: (layout) => call(OQTO_OPS.layoutUsage, { layout }),
    find: (q, opts) => call(OQTO_OPS.find, { q, ...opts }),
    media: () => call(OQTO_OPS.media),
    uploadMedia: async (slide, file, opts) =>
      call(OQTO_OPS.uploadMedia, {
        slide,
        name: opts?.name ?? file.name,
        overwrite: opts?.overwrite ?? false,
        base64: await fileToBase64(file),
      }),
    deleteMedia: (path, force) => call(OQTO_OPS.deleteMedia, { path, force: force ?? false }),

    previewSlide: (slide, opts) => html(OQTO_OPS.previewSlide, { slide, ...opts }),
    previewDeck: (playlist, flavor) => html(OQTO_OPS.previewDeck, { playlist, flavor: flavor ?? null }),
    previewLayout: (layout, flavor) => html(OQTO_OPS.previewLayout, { layout, flavor: flavor ?? null }),
    previewSample: (flavor, bust) => html(OQTO_OPS.previewSample, { flavor, bust: bust ?? 0 }),
    mediaUrl: async (path) => {
      const r = await call<{ base64: string; mime: string } | null>(OQTO_OPS.mediaUrl, { path });
      return r ? `data:${r.mime};base64,${r.base64}` : null;
    },
  };
}
