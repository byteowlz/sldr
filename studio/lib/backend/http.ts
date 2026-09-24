// Standalone backend: sldr-server over HTTP with the bearer token. Thin —
// the typed client in ../api does the work; previews are plain URLs.

import { api, deckPreviewUrl, layoutPreviewUrl, samplePreviewUrl, slidePreviewUrl } from "../api";
import type { Backend, Preview } from "./types";

const url = (u: string): Preview => ({ kind: "url", url: u });

export function httpBackend(): Backend {
  return {
    id: "http",
    canOpenExternal: true,

    slides: api.slides,
    slideDetail: api.slideDetail,
    saveSlideRaw: api.saveSlideRaw,
    saveSlideMeta: api.saveSlideMeta,
    createSlide: api.createSlide,
    playlists: api.playlists,
    createPlaylist: api.createPlaylist,
    updatePlaylist: api.updatePlaylist,
    layouts: api.layouts,
    layout: api.layout,
    saveZones: api.saveZones,
    flavors: api.flavors,
    getFlavor: api.getFlavor,
    saveFlavor: api.saveFlavor,
    build: api.build,

    zones: api.zones,
    layoutCandidates: api.layoutCandidates,
    slideUsage: api.slideUsage,
    usageIndex: api.usageIndex,
    layoutUsage: api.layoutUsage,
    find: api.find,
    media: api.media,
    uploadMedia: api.uploadMedia,
    deleteMedia: api.deleteMedia,

    previewSlide: async (slide, opts) =>
      url(slidePreviewUrl(slide, opts?.flavor, opts?.layout) + (opts?.bust ? `&v=${opts.bust}` : "")),
    previewDeck: async (playlist, flavor) => url(deckPreviewUrl(playlist, flavor)),
    previewLayout: async (layout, flavor) => url(layoutPreviewUrl(layout, flavor)),
    previewSample: async (flavor, bust) => url(samplePreviewUrl(flavor, bust)),
    mediaUrl: async (path) => api.mediaUrl(path),
  };
}
