// Open documents (ADR-0011): the tabs. A doc is any canonical file the user
// has open — a playlist, a slide, a layout, a flavor — plus its dirty flag.
// This is the only state the studio owns; it lives per user in localStorage
// and nothing about it ever enters a format the CLI can see.
//
// Deliberately tiny: an external store with `useSyncExternalStore`, no
// reducer framework. The deck board reads the playlist tabs as its columns.

import { useSyncExternalStore } from "react";

export type DocKind = "playlist" | "slide" | "layout" | "flavor";

export interface OpenDoc {
  kind: DocKind;
  /** Library name (playlist name, slide relative path, layout/flavor name). */
  id: string;
  /** Display title; defaults to the id's last segment. */
  title?: string;
  dirty?: boolean;
  /** A playlist being created that has no file yet. */
  draft?: boolean;
}

export interface DocsState {
  docs: OpenDoc[];
  /** `kind:id` of the active tab, or null. */
  active: string | null;
}

export const docKey = (d: Pick<OpenDoc, "kind" | "id">) => `${d.kind}:${d.id}`;

const STORAGE_KEY = "sldr:studio:docs";
const EMPTY: DocsState = { docs: [], active: null };

function load(): DocsState {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return EMPTY;
    const s = JSON.parse(raw) as DocsState;
    if (!Array.isArray(s.docs)) return EMPTY;
    // Dirty flags never survive a reload: unsaved edits are gone with the page.
    return { docs: s.docs.map((d) => ({ ...d, dirty: false })), active: s.active ?? null };
  } catch {
    return EMPTY;
  }
}

let state: DocsState = load();
const listeners = new Set<() => void>();

function set(next: DocsState) {
  state = next;
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(next));
  } catch {
    /* private mode etc. — tabs just don't persist */
  }
  listeners.forEach((l) => l());
}

export const docs = {
  get: () => state,
  subscribe(l: () => void) {
    listeners.add(l);
    return () => listeners.delete(l);
  },
  /** Open (or focus) a doc. Returns its key. */
  open(d: OpenDoc): string {
    const key = docKey(d);
    const exists = state.docs.some((x) => docKey(x) === key);
    set({
      docs: exists ? state.docs.map((x) => (docKey(x) === key ? { ...x, ...d, dirty: x.dirty || d.dirty } : x)) : [...state.docs, d],
      active: key,
    });
    return key;
  },
  activate(key: string | null) {
    set({ ...state, active: key });
  },
  close(key: string) {
    const idx = state.docs.findIndex((x) => docKey(x) === key);
    const rest = state.docs.filter((x) => docKey(x) !== key);
    const active =
      state.active === key ? (rest[Math.max(0, idx - 1)] ? docKey(rest[Math.max(0, idx - 1)]) : null) : state.active;
    set({ docs: rest, active });
  },
  /** Re-key a draft once it has a real name (playlist created). */
  rename(key: string, id: string, patch: Partial<OpenDoc> = {}) {
    let nextKey = key;
    const list = state.docs.map((x) => {
      if (docKey(x) !== key) return x;
      const nx = { ...x, id, ...patch };
      nextKey = docKey(nx);
      return nx;
    });
    set({ docs: list, active: state.active === key ? nextKey : state.active });
  },
  setDirty(key: string, dirty: boolean) {
    if (!state.docs.some((x) => docKey(x) === key && !!x.dirty !== dirty)) return;
    set({ ...state, docs: state.docs.map((x) => (docKey(x) === key ? { ...x, dirty } : x)) });
  },
  move(from: number, to: number) {
    const list = [...state.docs];
    const [d] = list.splice(from, 1);
    list.splice(to, 0, d);
    set({ ...state, docs: list });
  },
  anyDirty: () => state.docs.some((d) => d.dirty),
};

export function useDocs(): DocsState {
  return useSyncExternalStore(docs.subscribe, docs.get, docs.get);
}

export function useActiveDoc(): OpenDoc | null {
  const s = useDocs();
  return s.docs.find((d) => docKey(d) === s.active) ?? null;
}
