// Board model (ADR-0011): which columns are open and what each shows. The
// column list is UI state (per user, localStorage); decks and slides are
// canonical files read through the backend. Nothing here resolves a slide
// name — playlist entries are mapped to slides by the core usage index.

import type { Hit, Playlist, UsageIndex } from "@/lib/api";

export type ColumnSource = { kind: "deck"; name: string } | { kind: "search"; q: string };

/** One card in a column. `rel` is the slide's library path when the core
 * resolved it; null for a playlist entry that points at nothing. */
export interface Item {
  raw: string;
  rel: string | null;
}

export const colKey = (c: ColumnSource) => (c.kind === "deck" ? `deck:${c.name}` : `search:${c.q}`);

/** Map a playlist's entries to library slides using the usage index. */
export function deckItems(p: Playlist, usage: UsageIndex | undefined): Item[] {
  const items: Item[] = p.slides.map((raw) => ({ raw, rel: null }));
  if (!usage) return items;
  for (const [rel, refs] of Object.entries(usage.slides)) {
    for (const r of refs) {
      if ((r.playlist_name ?? r.name) === p.name) {
        const i = r.position - 1;
        if (items[i] && items[i].rel === null) items[i].rel = rel;
      }
    }
  }
  return items;
}

export const searchItems = (hits: Hit[] | undefined): Item[] =>
  (hits ?? []).map((h) => ({ raw: h.relative_path, rel: h.relative_path }));

/** What to write into a playlist for a slide inserted from the board. */
export const entryFor = (rel: string) => rel.replace(/\.md$/, "");

const KEY = "sldr:studio:board";

export function loadColumns(): ColumnSource[] {
  try {
    const v = JSON.parse(localStorage.getItem(KEY) ?? "[]");
    return Array.isArray(v) ? v : [];
  } catch {
    return [];
  }
}

export function saveColumns(cols: ColumnSource[]) {
  try {
    localStorage.setItem(KEY, JSON.stringify(cols));
  } catch {
    /* not persisted — fine */
  }
}
