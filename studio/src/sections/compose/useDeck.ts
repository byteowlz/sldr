// Deck (playlist) editing state and operations. The deck is a document in
// the open-docs store; this hook owns the working copy, dirty tracking,
// save/build, and the reference-only edits (insert / remove / reorder —
// never a copy of a slide file).

import { useCallback, useEffect, useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import type { Playlist } from "@/lib/api";
import { useBackend } from "@/lib/backend";
import { docs, docKey } from "@/lib/docs/store";
import { baseName } from "./Browser";

export type Deck = Playlist & { isNew?: boolean };

export interface LogMsg {
  kind: "system" | "action";
  text: string;
  ts: string;
  ref?: string;
}

const now = () => new Date().toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });

export function useDeck() {
  const backend = useBackend();
  const qc = useQueryClient();
  const [deck, setDeck] = useState<Deck | null>(null);
  const [dirty, setDirty] = useState(false);
  const [activeIdx, setActiveIdx] = useState(0);
  const [flavor, setFlavor] = useState<string | null>(null);
  const [log, setLog] = useState<LogMsg[]>([]);

  const pushLog = useCallback(
    (kind: LogMsg["kind"], text: string, ref?: string) =>
      setLog((l) => [...l.slice(-60), { kind, text, ts: now(), ref }]),
    [],
  );

  // Mirror the dirty flag onto the open tab so the tab strip shows it.
  useEffect(() => {
    if (deck) docs.setDirty(docKey({ kind: "playlist", id: deck.name }), dirty);
  }, [deck, dirty]);

  const open = useCallback(
    (p: Playlist) => {
      if (dirty && !confirm("Discard unsaved deck changes?")) return false;
      setDeck({ ...p });
      setDirty(false);
      setActiveIdx(0);
      setFlavor(p.flavor ?? null);
      docs.open({ kind: "playlist", id: p.name, title: p.title ?? p.name });
      pushLog("system", `opened deck ${p.name} · ${p.slides.length} slides`);
      return true;
    },
    [dirty, pushLog],
  );

  const create = useCallback(() => {
    if (dirty && !confirm("Discard unsaved deck changes?")) return false;
    setDeck({ name: "untitled", slides: [], isNew: true });
    setDirty(true);
    setActiveIdx(0);
    docs.open({ kind: "playlist", id: "untitled", title: "untitled", draft: true, dirty: true });
    pushLog("system", "new deck — name it in the top bar, drag slides into the timeline");
    return true;
  }, [dirty, pushLog]);

  const mutate = useCallback((fn: (d: Deck) => Deck) => {
    setDeck((d) => (d ? fn(d) : d));
    setDirty(true);
  }, []);

  const rename = useCallback(
    (name: string) => {
      if (!deck || name === deck.name) return;
      docs.rename(docKey({ kind: "playlist", id: deck.name }), name, { title: name });
      mutate((d) => ({ ...d, name }));
    },
    [deck, mutate],
  );

  const insertAt = useCallback(
    (name: string, idx: number) => {
      if (!deck) return;
      mutate((d) => {
        const s = [...d.slides];
        s.splice(idx, 0, name);
        return { ...d, slides: s };
      });
      setActiveIdx(idx);
      pushLog("action", `+ ${baseName(name)} at ${idx + 1}`, name);
    },
    [deck, mutate, pushLog],
  );

  const removeAt = useCallback(
    (idx: number) => {
      if (!deck) return;
      const name = deck.slides[idx];
      mutate((d) => ({ ...d, slides: d.slides.filter((_, i) => i !== idx) }));
      setActiveIdx((i) => Math.max(0, Math.min(i, deck.slides.length - 2)));
      pushLog("action", `− ${baseName(name)} removed`);
    },
    [deck, mutate, pushLog],
  );

  const reorder = useCallback(
    (from: number, toInsert: number) => {
      if (!deck || from === toInsert || from + 1 === toInsert) return;
      mutate((d) => {
        const s = [...d.slides];
        const [x] = s.splice(from, 1);
        s.splice(from < toInsert ? toInsert - 1 : toInsert, 0, x);
        return { ...d, slides: s };
      });
      setActiveIdx(from < toInsert ? toInsert - 1 : toInsert);
    },
    [deck, mutate],
  );

  const duplicateAt = useCallback(
    (idx: number) => deck && insertAt(deck.slides[idx], idx + 1),
    [deck, insertAt],
  );

  const save = useMutation({
    mutationFn: async () => {
      if (!deck) throw new Error("no deck");
      const name = deck.name.trim();
      if (!name || name === "untitled") throw new Error("name the deck first (click its name in the bar)");
      const body: Playlist = { ...deck, name, flavor };
      if (deck.isNew) return backend.createPlaylist(body);
      return backend.updatePlaylist(name, body);
    },
    onSuccess: () => {
      setDirty(false);
      setDeck((d) => (d ? { ...d, isNew: false } : d));
      if (deck) docs.open({ kind: "playlist", id: deck.name, title: deck.title ?? deck.name, draft: false, dirty: false });
      qc.invalidateQueries({ queryKey: ["playlists"] });
      qc.invalidateQueries({ queryKey: ["usage"] });
      pushLog("action", `deck saved · ${deck?.name}`);
    },
    onError: (e) => pushLog("system", `save failed: ${(e as Error).message}`),
  });

  const build = useMutation({
    mutationFn: () => backend.build(deck!.name, flavor ?? undefined),
    onSuccess: (r) => pushLog("action", `built → ${r.html_path}`),
    onError: (e) => pushLog("system", `build failed: ${(e as Error).message}`),
  });

  return {
    deck,
    dirty,
    activeIdx,
    setActiveIdx,
    flavor,
    setFlavor: (f: string | null) => {
      setFlavor(f);
      if (deck) mutate((d) => ({ ...d, flavor: f }));
    },
    log,
    pushLog,
    open,
    create,
    rename,
    insertAt,
    removeAt,
    reorder,
    duplicateAt,
    save,
    build,
    canBuild: !!deck && !deck.isNew && !dirty,
  };
}
