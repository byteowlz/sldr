// Deck board (ADR-0011): every open deck is a column, the leftmost is the
// working deck and the only one that writes. Other columns are sources —
// decks or searches — and inserting from them adds a playlist reference,
// never a copy. Hover a slide to see every column that shares it.

import { useEffect, useMemo, useState } from "react";
import { useMutation, useQueries, useQuery, useQueryClient } from "@tanstack/react-query";
import { useBackend } from "@/lib/backend";
import { docs } from "@/lib/docs/store";
import type { Playlist } from "@/lib/api";
import { TopBar, type Chrome } from "../components/chrome";
import { Column } from "./board/Column";
import { colKey, deckItems, entryFor, loadColumns, saveColumns, searchItems, type ColumnSource, type Item } from "./board/model";

export function Board({ chrome }: { chrome: Chrome }) {
  const backend = useBackend();
  const qc = useQueryClient();
  const playlists = useQuery({ queryKey: ["playlists"], queryFn: backend.playlists });
  const usage = useQuery({ queryKey: ["usage", "index"], queryFn: backend.usageIndex });
  const slides = useQuery({ queryKey: ["slides"], queryFn: backend.slides });

  const [cols, setCols] = useState<ColumnSource[]>(loadColumns);
  useEffect(() => saveColumns(cols), [cols]);

  // Seed with the first deck so the board is never empty on first open.
  useEffect(() => {
    if (cols.length === 0 && playlists.data?.length) setCols([{ kind: "deck", name: playlists.data[0].name }]);
  }, [cols.length, playlists.data]);

  const byName = useMemo(() => new Map((playlists.data ?? []).map((p) => [p.name, p])), [playlists.data]);
  const titleOf = useMemo(
    () => new Map((slides.data ?? []).map((s) => [s.relative_path, String(s.metadata?.title ?? "")])),
    [slides.data],
  );
  const usesOf = (rel: string | null) => (rel ? (usage.data?.slides[rel]?.length ?? 0) : 0);

  const searches = useQueries({
    queries: cols
      .filter((c): c is { kind: "search"; q: string } => c.kind === "search")
      .map((c) => ({ queryKey: ["find", c.q], queryFn: () => backend.find(c.q, { limit: 40 }) })),
  });
  const searchByQ = new Map(
    cols.filter((c) => c.kind === "search").map((c, i) => [(c as { q: string }).q, searches[i]]),
  );

  // ---- working deck (leftmost deck column) ----
  const workingSrc = cols[0]?.kind === "deck" ? cols[0] : null;
  const workingPl = workingSrc ? byName.get(workingSrc.name) : undefined;
  const [items, setItems] = useState<Item[]>([]);
  const [dirty, setDirty] = useState(false);
  const [activeIdx, setActiveIdx] = useState(0);
  useEffect(() => {
    if (!dirty && workingPl) setItems(deckItems(workingPl, usage.data));
  }, [workingPl, usage.data, dirty]);
  useEffect(() => {
    if (workingPl) docs.open({ kind: "playlist", id: workingPl.name, title: workingPl.title ?? workingPl.name });
  }, [workingPl]);
  useEffect(() => {
    if (workingPl) docs.setDirty(`playlist:${workingPl.name}`, dirty);
  }, [workingPl, dirty]);
  const workingSet = useMemo(() => new Set(items.map((i) => i.rel).filter((r): r is string => !!r)), [items]);
  const flavor = workingPl?.flavor ?? null;

  const edit = (next: Item[], active?: number) => {
    setItems(next);
    setDirty(true);
    if (active !== undefined) setActiveIdx(active);
  };
  const insert = (rel: string, at: number) => {
    const next = [...items];
    next.splice(at, 0, { raw: entryFor(rel), rel });
    edit(next, at);
  };

  const save = useMutation({
    mutationFn: async () => {
      if (!workingPl) throw new Error("no working deck");
      const body: Playlist = { ...workingPl, slides: items.map((i) => i.raw) };
      return backend.updatePlaylist(workingPl.name, body);
    },
    onSuccess: () => {
      setDirty(false);
      qc.invalidateQueries({ queryKey: ["playlists"] });
      qc.invalidateQueries({ queryKey: ["usage"] });
    },
  });

  const [hovered, setHovered] = useState<string | null>(null);

  const setWorking = (idx: number) => {
    if (dirty && !confirm("Discard unsaved changes to the working deck?")) return;
    setDirty(false);
    setActiveIdx(0);
    setCols((c) => {
      const next = [...c];
      const [picked] = next.splice(idx, 1);
      return [picked, ...next];
    });
  };
  const closeCol = (idx: number) => {
    if (idx === 0 && dirty && !confirm("Close the working deck with unsaved changes?")) return;
    if (idx === 0) setDirty(false);
    setCols((c) => c.filter((_, i) => i !== idx));
  };
  const addCol = (c: ColumnSource) => setCols((cs) => (cs.some((x) => colKey(x) === colKey(c)) ? cs : [...cs, c]));

  // ---- keyboard: save, remove, navigate ----
  useEffect(() => {
    const h = (e: KeyboardEvent) => {
      const typing = !!(e.target as HTMLElement).closest("input, textarea, [contenteditable]");
      if ((e.metaKey || e.ctrlKey) && e.key === "s" && dirty) {
        e.preventDefault();
        save.mutate();
        return;
      }
      if (typing || !workingPl) return;
      if ((e.key === "Backspace" || e.key === "Delete") && items.length) {
        e.preventDefault();
        edit(items.filter((_, i) => i !== activeIdx), Math.max(0, Math.min(activeIdx, items.length - 2)));
      }
      if (e.key === "ArrowUp") setActiveIdx((i) => Math.max(0, i - 1));
      if (e.key === "ArrowDown") setActiveIdx((i) => Math.min(items.length - 1, i + 1));
    };
    window.addEventListener("keydown", h);
    return () => window.removeEventListener("keydown", h);
  });

  const [q, setQ] = useState("");
  const openDecks = new Set(cols.filter((c) => c.kind === "deck").map((c) => (c as { name: string }).name));
  const hoverInfo = hovered
    ? `${hovered} · ${titleOf.get(hovered) || "untitled"} · in ${usesOf(hovered)} deck${usesOf(hovered) === 1 ? "" : "s"}`
    : workingPl
      ? `${items.length} references · zero copies`
      : "open a deck to start";

  const crumb = (
    <>
      <span className="sl-crumb-sep">/</span>
      <span className="sl-crumb">board</span>
      {workingPl && (
        <span className={dirty ? "sl-sync sl-sync-mod ml-1" : "sl-sync ml-1"}>
          <span className="sl-sync-dot" />
          {dirty ? `${workingPl.name} unsaved` : `${workingPl.name} saved`}
        </span>
      )}
    </>
  );

  return (
    <div className="sl-root grid h-svh w-svw max-w-svw grid-rows-[40px_minmax(0,1fr)_24px] overflow-hidden">
      <TopBar chrome={chrome} crumb={crumb} />
      <div className="sl-board">
        {cols.map((c, i) => {
          const key = colKey(c);
          if (c.kind === "deck") {
            const p = byName.get(c.name);
            if (!p) return null;
            const isWorking = i === 0;
            const colItems = isWorking ? items : deckItems(p, usage.data);
            return (
              <Column
                key={key}
                kind="deck"
                title={p.title ?? p.name}
                meta={`${p.name}${p.flavor ? ` · ${p.flavor}` : ""}`}
                items={colItems}
                working={isWorking}
                dirty={isWorking && dirty}
                saving={save.isPending}
                flavor={flavor}
                activeIdx={activeIdx}
                workingSet={workingSet}
                usesOf={usesOf}
                hovered={hovered}
                onHover={setHovered}
                loading={usage.isLoading}
                onCardClick={(idx, item) => {
                  if (isWorking) setActiveIdx(idx);
                  else if (workingPl && item.rel && !workingSet.has(item.rel)) insert(item.rel, Math.min(activeIdx + 1, items.length));
                }}
                onDropAt={
                  isWorking
                    ? (idx, payload) => {
                        if (payload.startsWith("file:")) insert(payload.slice(5), idx);
                        else if (payload.startsWith("move:")) {
                          const from = Number(payload.slice(5));
                          if (from === idx || from + 1 === idx) return;
                          const next = [...items];
                          const [m] = next.splice(from, 1);
                          const to = from < idx ? idx - 1 : idx;
                          next.splice(to, 0, m);
                          edit(next, to);
                        }
                      }
                    : undefined
                }
                onMakeWorking={isWorking ? undefined : () => setWorking(i)}
                onClose={() => closeCol(i)}
                onSave={() => save.mutate()}
              />
            );
          }
          const res = searchByQ.get(c.q);
          return (
            <Column
              key={key}
              kind="search"
              title={`“${c.q}”`}
              meta="title · tags · body"
              items={searchItems(res?.data)}
              working={false}
              flavor={flavor}
              activeIdx={-1}
              workingSet={workingSet}
              usesOf={usesOf}
              hovered={hovered}
              onHover={setHovered}
              loading={res?.isLoading}
              onCardClick={(_, item) => {
                if (workingPl && item.rel && !workingSet.has(item.rel)) insert(item.rel, Math.min(activeIdx + 1, items.length));
              }}
              onClose={() => closeCol(i)}
            />
          );
        })}

        <aside className="sl-board-add">
          <div className="sl-label">add column</div>
          <form
            onSubmit={(e) => {
              e.preventDefault();
              if (q.trim()) addCol({ kind: "search", q: q.trim() });
              setQ("");
            }}
          >
            <input className="sl-input" placeholder="search the library…" value={q} onChange={(e) => setQ(e.target.value)} />
          </form>
          <div className="sl-microlabel mt-2">decks</div>
          <div className="flex min-h-0 flex-col overflow-y-auto">
            {(playlists.data ?? [])
              .filter((p) => !openDecks.has(p.name))
              .map((p) => (
                <button key={p.name} className="sl-row sl-indent-0 text-left" onClick={() => addCol({ kind: "deck", name: p.name })}>
                  <span className="sl-row-ico sl-ico-deck">▣</span>
                  <span className="sl-row-name">{p.title ?? p.name}</span>
                  <span className="sl-row-meta">{p.slides.length}</span>
                </button>
              ))}
          </div>
        </aside>
      </div>
      <div className="sl-board-status sl-microlabel">
        <span className="truncate" style={{ color: hovered ? "var(--sl-fg)" : undefined }}>
          {hoverInfo}
        </span>
        <span className="flex-1" />
        <span className="hidden md:inline">hover: shared across decks · click a source: insert after the active slide · ← work on this: swap left</span>
      </div>
    </div>
  );
}
