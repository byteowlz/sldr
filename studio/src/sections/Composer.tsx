import { useEffect, useMemo, useRef, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { ExternalLink, Hammer, Loader2, Save } from "lucide-react";
import {
  api,
  deckPreviewUrl,
  slidePreviewUrl,
  type Playlist,
  type SlideSummary,
} from "@/lib/api";
import { cn } from "@/lib/utils";
import { SlideFrame } from "../components/shell";
import { TopBar, FlavorChip, type Chrome } from "../components/chrome";
import { CodeArea } from "../components/code-area";

type Deck = Playlist & { isNew?: boolean };
interface LogMsg {
  kind: "system" | "action";
  text: string;
  ts: string;
  ref?: string;
}

const now = () =>
  new Date().toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });

// ---------------------------------------------------------------------------
// Browser (FS / TAGS / RECENT)
// ---------------------------------------------------------------------------

function Row({
  indent = 0,
  caret,
  icon,
  iconClass,
  name,
  meta,
  active,
  draggable,
  onClick,
  onDoubleClick,
  onDragStart,
  title,
}: {
  indent?: number;
  caret?: "open" | "closed" | null;
  icon: string;
  iconClass?: string;
  name: string;
  meta?: string | number;
  active?: boolean;
  draggable?: boolean;
  onClick?: () => void;
  onDoubleClick?: () => void;
  onDragStart?: (e: React.DragEvent) => void;
  title?: string;
}) {
  return (
    <div
      className={cn("sl-row", `sl-indent-${Math.min(indent, 3)}`, active && "sl-row-active")}
      draggable={draggable}
      onClick={onClick}
      onDoubleClick={onDoubleClick}
      onDragStart={onDragStart}
      title={title}
    >
      <span className="sl-row-caret">{caret ? (caret === "open" ? "▾" : "▸") : ""}</span>
      <span className={cn("sl-row-ico", iconClass)}>{icon}</span>
      <span className="sl-row-name">{name}</span>
      <span className="sl-row-meta">{meta}</span>
    </div>
  );
}

interface TreeDir {
  dirs: Record<string, TreeDir>;
  slides: SlideSummary[];
}

function buildTree(slides: SlideSummary[]): TreeDir {
  const root: TreeDir = { dirs: {}, slides: [] };
  for (const s of slides) {
    const parts = s.relative_path.split("/");
    let cur = root;
    for (const p of parts.slice(0, -1)) {
      cur = cur.dirs[p] ??= { dirs: {}, slides: [] };
    }
    cur.slides.push(s);
  }
  return root;
}

const slideDate = (s: SlideSummary) =>
  (s.metadata?.modified as string) || (s.metadata?.created as string) || "";
const slideTags = (s: SlideSummary) =>
  Array.isArray(s.metadata?.tags) ? (s.metadata.tags as string[]) : [];
const baseName = (n: string) => n.split("/").pop() ?? n;

function Browser({
  slides,
  playlists,
  activeDeck,
  libSel,
  onOpenDeck,
  onNewDeck,
  onSelectSlide,
}: {
  slides: SlideSummary[];
  playlists: Playlist[];
  activeDeck: string | null;
  libSel: string | null;
  onOpenDeck: (p: Playlist) => void;
  onNewDeck: () => void;
  onSelectSlide: (name: string) => void;
}) {
  const [view, setView] = useState<"fs" | "tags" | "recent">("fs");
  const [q, setQ] = useState("");
  const [open, setOpen] = useState<Record<string, boolean>>({ "": true, "/decks": true });
  const toggle = (k: string) => setOpen((o) => ({ ...o, [k]: !(o[k] ?? false) }));

  const ql = q.toLowerCase();
  const match = (s: SlideSummary) =>
    !ql ||
    s.name.toLowerCase().includes(ql) ||
    String(s.metadata?.title ?? "").toLowerCase().includes(ql);

  const tree = useMemo(() => buildTree(slides), [slides]);

  const slideRow = (s: SlideSummary, indent: number, key?: string) => (
    <Row
      key={key ?? s.name}
      indent={indent}
      icon="▪"
      iconClass="sl-ico-slide"
      name={baseName(s.name)}
      meta={slideDate(s).slice(5).replace("-", "·") || undefined}
      active={libSel === s.name}
      draggable
      onClick={() => onSelectSlide(s.name)}
      onDragStart={(e) => {
        e.dataTransfer.setData("text/plain", "file:" + s.name);
        e.dataTransfer.effectAllowed = "copy";
      }}
      title={`${s.relative_path}\ndrag into the timeline to add`}
    />
  );

  const renderDir = (dir: TreeDir, path: string, depth: number): React.ReactNode[] => {
    const out: React.ReactNode[] = [];
    for (const dname of Object.keys(dir.dirs).sort()) {
      const child = dir.dirs[dname];
      const cpath = `${path}/${dname}`;
      const isOpen = open[cpath] ?? depth < 1;
      const count = child.slides.length + Object.keys(child.dirs).length;
      const anyVisible =
        !ql || child.slides.some(match) || Object.keys(child.dirs).length > 0;
      if (!anyVisible) continue;
      out.push(
        <Row
          key={cpath}
          indent={depth}
          caret={isOpen ? "open" : "closed"}
          icon={isOpen ? "▾" : "▸"}
          iconClass="sl-ico-folder"
          name={dname + "/"}
          meta={count}
          onClick={() => toggle(cpath)}
        />,
      );
      if (isOpen) out.push(...renderDir(child, cpath, depth + 1));
    }
    out.push(...dir.slides.filter(match).map((s) => slideRow(s, depth)));
    return out;
  };

  const tagGroups = useMemo(() => {
    const m: Record<string, SlideSummary[]> = {};
    for (const s of slides) for (const t of slideTags(s)) (m[t] ??= []).push(s);
    return Object.entries(m).sort((a, b) => b[1].length - a[1].length);
  }, [slides]);

  const recentGroups = useMemo(() => {
    const dated = slides
      .map((s) => ({ s, d: slideDate(s) }))
      .sort((a, b) => b.d.localeCompare(a.d));
    const m: Record<string, SlideSummary[]> = {};
    for (const { s, d } of dated) {
      (m[d ? d.slice(0, 7) : "undated"] ??= []).push(s);
    }
    return Object.entries(m);
  }, [slides]);

  return (
    <aside
      className="flex min-h-0 flex-col overflow-hidden border-r"
      style={{ borderColor: "var(--sl-border)", background: "var(--sl-panel)" }}
    >
      <div
        className="grid grid-cols-3 border-b"
        style={{ borderColor: "var(--sl-border)" }}
      >
        {(["fs", "tags", "recent"] as const).map((v) => (
          <button
            key={v}
            className={cn("sl-tab !py-2 text-center", view === v && "sl-tab-active")}
            style={{ borderBottomWidth: 2 }}
            onClick={() => setView(v)}
          >
            {v}
          </button>
        ))}
      </div>
      <div className="flex items-center gap-1.5 px-2.5 py-2">
        <div className="relative flex-1">
          <span
            className="pointer-events-none absolute left-2 top-1/2 -translate-y-1/2 text-[11px]"
            style={{ color: "var(--sl-dim)" }}
          >
            /
          </span>
          <input
            className="sl-input pl-6"
            placeholder={`filter ${view}…`}
            value={q}
            onChange={(e) => setQ(e.target.value)}
          />
        </div>
        <button className="sl-btn !h-6 !px-2" title="new deck" onClick={onNewDeck}>
          +deck
        </button>
      </div>
      <div className="min-h-0 flex-1 overflow-y-auto pb-3">
        {view === "fs" && (
          <>
            <Row
              indent={0}
              caret={open["/decks"] ? "open" : "closed"}
              icon={open["/decks"] ? "▾" : "▸"}
              iconClass="sl-ico-folder"
              name="decks/"
              meta={playlists.length}
              onClick={() => toggle("/decks")}
            />
            {open["/decks"] &&
              playlists
                .filter((p) => !ql || p.name.toLowerCase().includes(ql))
                .map((p) => (
                  <Row
                    key={p.name}
                    indent={1}
                    icon="▣"
                    iconClass="sl-ico-deck"
                    name={p.name}
                    meta={p.slides.length}
                    active={activeDeck === p.name}
                    onDoubleClick={() => onOpenDeck(p)}
                    onClick={() => onOpenDeck(p)}
                    title={`${p.slides.length} slides · click to open`}
                  />
                ))}
            {renderDir(tree, "", 0)}
          </>
        )}
        {view === "tags" &&
          tagGroups.map(([tag, list]) => {
            const visible = list.filter(match);
            if (!visible.length) return null;
            const isOpen = open["#" + tag] ?? false;
            return (
              <div key={tag}>
                <Row
                  indent={0}
                  caret={isOpen ? "open" : "closed"}
                  icon="#"
                  iconClass="sl-ico-folder"
                  name={tag}
                  meta={list.length}
                  onClick={() => toggle("#" + tag)}
                />
                {isOpen && visible.map((s) => slideRow(s, 1, tag + s.name))}
              </div>
            );
          })}
        {view === "recent" &&
          recentGroups.map(([month, list]) => {
            const visible = list.filter(match);
            if (!visible.length) return null;
            const isOpen = open["@" + month] ?? month !== "undated";
            return (
              <div key={month}>
                <Row
                  indent={0}
                  caret={isOpen ? "open" : "closed"}
                  icon="◷"
                  name={month}
                  meta={list.length}
                  onClick={() => toggle("@" + month)}
                />
                {isOpen && visible.map((s) => slideRow(s, 1, month + s.name))}
              </div>
            );
          })}
      </div>
    </aside>
  );
}

// ---------------------------------------------------------------------------
// Composer
// ---------------------------------------------------------------------------

export function Composer({ chrome }: { chrome: Chrome }) {
  const qc = useQueryClient();
  const slides = useQuery({ queryKey: ["slides"], queryFn: api.slides });
  const playlists = useQuery({ queryKey: ["playlists"], queryFn: api.playlists });
  const layouts = useQuery({ queryKey: ["layouts"], queryFn: api.layouts });

  const [deck, setDeck] = useState<Deck | null>(null);
  const [deckDirty, setDeckDirty] = useState(false);
  const [activeIdx, setActiveIdx] = useState(0);
  const [libSel, setLibSel] = useState<string | null>(null);
  const [flavor, setFlavor] = useState<string | null>(null);
  const [sourceOpen, setSourceOpen] = useState(true);
  const [versions, setVersions] = useState<Record<string, number>>({});
  const [log, setLog] = useState<LogMsg[]>([]);
  const [ctx, setCtx] = useState<{ x: number; y: number; idx: number } | null>(null);
  const [dragOverIdx, setDragOverIdx] = useState<number | null>(null);
  const [dragIdx, setDragIdx] = useState<number | null>(null);
  const [newOpen, setNewOpen] = useState(false);
  const [newName, setNewName] = useState("");

  const pushLog = (kind: LogMsg["kind"], text: string, ref?: string) =>
    setLog((l) => [...l.slice(-60), { kind, text, ts: now(), ref }]);

  // seed the activity log once the library is known
  const seeded = useRef(false);
  useEffect(() => {
    if (!seeded.current && slides.data && playlists.data) {
      seeded.current = true;
      pushLog(
        "system",
        `studio ready · ${slides.data.length} slides · ${playlists.data.length} decks`,
      );
    }
  }, [slides.data, playlists.data]);

  // ---- stage selection ----
  const stageName = libSel ?? deck?.slides[activeIdx] ?? null;
  const stageInDeck = libSel === null && !!deck?.slides.length;
  const previewFlavor = flavor ?? undefined;
  const v = stageName ? (versions[stageName] ?? 0) : 0;

  const detail = useQuery({
    queryKey: ["slideDetail", stageName, v],
    queryFn: () => api.slideDetail(stageName!),
    enabled: !!stageName,
  });
  const [raw, setRaw] = useState("");
  const [rawDirty, setRawDirty] = useState(false);
  useEffect(() => {
    setRaw(detail.data?.raw ?? "");
    setRawDirty(false);
  }, [detail.data?.raw, stageName]);

  // ---- deck ops ----
  const openDeck = (p: Playlist) => {
    if (deckDirty && !confirm("Discard unsaved deck changes?")) return;
    setDeck({ ...p });
    setDeckDirty(false);
    setActiveIdx(0);
    setLibSel(null);
    setFlavor(p.flavor ?? null);
    pushLog("system", `opened deck ${p.name} · ${p.slides.length} slides`);
  };
  const newDeck = () => {
    if (deckDirty && !confirm("Discard unsaved deck changes?")) return;
    setDeck({ name: "untitled", slides: [], isNew: true });
    setDeckDirty(true);
    setActiveIdx(0);
    setLibSel(null);
    pushLog("system", "new deck — name it in the top bar, drag slides into the timeline");
  };
  const mutateDeck = (fn: (d: Deck) => Deck) => {
    setDeck((d) => (d ? fn(d) : d));
    setDeckDirty(true);
  };
  const insertAt = (name: string, idx: number) => {
    if (!deck) return;
    mutateDeck((d) => {
      const s = [...d.slides];
      s.splice(idx, 0, name);
      return { ...d, slides: s };
    });
    setLibSel(null);
    setActiveIdx(idx);
    pushLog("action", `+ ${baseName(name)} at ${idx + 1}`, name);
  };
  const removeAt = (idx: number) => {
    if (!deck) return;
    const name = deck.slides[idx];
    mutateDeck((d) => ({ ...d, slides: d.slides.filter((_, i) => i !== idx) }));
    setActiveIdx((i) => Math.max(0, Math.min(i, (deck.slides.length ?? 1) - 2)));
    pushLog("action", `− ${baseName(name)} removed`);
  };
  const reorder = (from: number, toInsert: number) => {
    if (!deck || from === toInsert || from + 1 === toInsert) return;
    mutateDeck((d) => {
      const s = [...d.slides];
      const [x] = s.splice(from, 1);
      s.splice(from < toInsert ? toInsert - 1 : toInsert, 0, x);
      return { ...d, slides: s };
    });
    setActiveIdx(from < toInsert ? toInsert - 1 : toInsert);
  };
  const duplicateAt = (idx: number) => {
    if (!deck) return;
    insertAt(deck.slides[idx], idx + 1);
  };

  const saveDeck = useMutation({
    mutationFn: async () => {
      if (!deck) throw new Error("no deck");
      const name = deck.name.trim();
      if (!name || name === "untitled") throw new Error("name the deck first (click its name in the bar)");
      const body: Playlist = { ...deck, name, flavor };
      if (deck.isNew) return api.createPlaylist(body);
      return api.updatePlaylist(name, body);
    },
    onSuccess: () => {
      setDeckDirty(false);
      setDeck((d) => (d ? { ...d, isNew: false } : d));
      qc.invalidateQueries({ queryKey: ["playlists"] });
      pushLog("action", `deck saved · ${deck?.name}`);
    },
    onError: (e) => pushLog("system", `save failed: ${(e as Error).message}`),
  });

  const build = useMutation({
    mutationFn: () => api.build(deck!.name),
    onSuccess: (r) => pushLog("action", `built → ${r.html_path}`),
    onError: (e) => pushLog("system", `build failed: ${(e as Error).message}`),
  });

  const saveSlide = useMutation({
    mutationFn: () => api.saveSlideRaw(stageName!, raw),
    onSuccess: (d) => {
      setRawDirty(false);
      setVersions((m) => ({ ...m, [d.name]: (m[d.name] ?? 0) + 1 }));
      qc.invalidateQueries({ queryKey: ["slides"] });
      pushLog("action", `saved ${baseName(d.name)}`, d.name);
    },
    onError: (e) => pushLog("system", `slide save failed: ${(e as Error).message}`),
  });

  const saveMeta = useMutation({
    mutationFn: (meta: Record<string, unknown>) =>
      api.saveSlideMeta(stageName!, { ...(detail.data?.metadata ?? {}), ...meta }),
    onSuccess: (d) => {
      setVersions((m) => ({ ...m, [d.name]: (m[d.name] ?? 0) + 1 }));
      qc.invalidateQueries({ queryKey: ["slides"] });
      qc.invalidateQueries({ queryKey: ["slideDetail", d.name] });
      pushLog("action", `metadata updated · ${baseName(d.name)}`);
    },
  });

  const createSlide = useMutation({
    mutationFn: (name: string) => api.createSlide(name),
    onSuccess: (d) => {
      qc.invalidateQueries({ queryKey: ["slides"] });
      if (deck) insertAt(d.name, deck.slides.length);
      else setLibSel(d.name);
      setSourceOpen(true);
      pushLog("action", `created ${d.name}`, d.name);
    },
    onError: (e) => pushLog("system", `create failed: ${(e as Error).message}`),
  });

  // ---- keyboard ----
  useEffect(() => {
    const h = (e: KeyboardEvent) => {
      const t = e.target as HTMLElement;
      const typing = !!t.closest("input, textarea, [contenteditable]");
      const mod = e.metaKey || e.ctrlKey;
      if (mod && e.key === "b") {
        e.preventDefault();
        if (deck && !deckDirty && !deck.isNew) build.mutate();
        return;
      }
      if (mod && e.key === "/") {
        e.preventDefault();
        setSourceOpen((s) => !s);
        return;
      }
      if (mod && e.key === "s" && !typing) {
        e.preventDefault();
        if (rawDirty) saveSlide.mutate();
        else if (deckDirty) saveDeck.mutate();
        return;
      }
      if (typing) return;
      if (e.key === "ArrowLeft" && deck?.slides.length) {
        setLibSel(null);
        setActiveIdx((i) => Math.max(0, i - 1));
      }
      if (e.key === "ArrowRight" && deck?.slides.length) {
        setLibSel(null);
        setActiveIdx((i) => Math.min(deck.slides.length - 1, i + 1));
      }
      if ((e.key === "Delete" || e.key === "Backspace") && stageInDeck) {
        removeAt(activeIdx);
      }
    };
    window.addEventListener("keydown", h);
    return () => window.removeEventListener("keydown", h);
  });

  // ---- strip DnD ----
  const dropAt = (idx: number, e: React.DragEvent) => {
    e.preventDefault();
    const payload = e.dataTransfer.getData("text/plain");
    if (payload.startsWith("file:")) insertAt(payload.slice(5), idx);
    else if (dragIdx !== null) reorder(dragIdx, idx);
    setDragIdx(null);
    setDragOverIdx(null);
  };

  const est = deck ? Math.round(deck.slides.length * 1.25) : 0;
  const meta = detail.data?.metadata ?? {};

  // ---- bar ----
  const crumb = (
    <>
      <span className="sl-crumb-sep">/</span>
      {deck ? (
        <span
          className="sl-crumb sl-crumb-edit"
          contentEditable={!!deck.isNew}
          suppressContentEditableWarning
          spellCheck={false}
          title={deck.isNew ? "name this deck" : `deck · ${deck.name}`}
          onBlur={(e) => {
            const n = (e.target as HTMLElement).textContent?.trim() || "untitled";
            if (n !== deck.name) mutateDeck((d) => ({ ...d, name: n }));
          }}
        >
          {deck.name}
        </span>
      ) : (
        <span className="sl-crumb">no deck open</span>
      )}
      {deck && (
        <span className={cn("sl-sync ml-1", deckDirty && "sl-sync-mod")}>
          <span className="sl-sync-dot" />
          {deckDirty ? "unsaved" : "saved"}
        </span>
      )}
    </>
  );

  const extras = (
    <>
      <button
        className={cn("sl-btn sl-btn-ghost", sourceOpen && "sl-btn-active")}
        onClick={() => setSourceOpen((s) => !s)}
        title="toggle source · ⌘/"
      >
        {"{ }"} source
      </button>
      <FlavorChip
        value={flavor}
        onChange={(f) => {
          setFlavor(f);
          if (deck) mutateDeck((d) => ({ ...d, flavor: f }));
        }}
      />
      {deck && (deckDirty || deck.isNew) && (
        <button
          className="sl-btn"
          style={{ color: "var(--sl-warn)", borderColor: "var(--sl-warn)" }}
          onClick={() => saveDeck.mutate()}
          disabled={saveDeck.isPending}
          title="save deck · ⌘S"
        >
          {saveDeck.isPending ? <Loader2 className="size-3 animate-spin" /> : <Save className="size-3" />}
          save
        </button>
      )}
      <button
        className="sl-btn"
        disabled={!deck || deck.isNew || deckDirty}
        title={deckDirty ? "save first" : "open the presenter"}
        onClick={() => window.open(deckPreviewUrl(deck!.name, flavor ?? undefined), "_blank")}
      >
        present <ExternalLink className="size-3" />
      </button>
      <button
        className="sl-btn sl-btn-build"
        disabled={!deck || deck.isNew || deckDirty || build.isPending}
        title={deckDirty ? "save first" : "build · ⌘B"}
        onClick={() => build.mutate()}
      >
        {build.isPending ? <Loader2 className="size-3 animate-spin" /> : <Hammer className="size-3" />}
        build
      </button>
    </>
  );

  return (
    <div className="sl-root grid h-svh w-svw max-w-svw overflow-hidden grid-rows-[40px_minmax(0,1fr)_132px]">
      <TopBar chrome={chrome} crumb={crumb} extras={extras} />

      {/* main */}
      <div className="grid min-h-0 min-w-0 grid-cols-[272px_minmax(0,1fr)_290px]">
        <Browser
          slides={slides.data ?? []}
          playlists={playlists.data ?? []}
          activeDeck={deck?.name ?? null}
          libSel={libSel}
          onOpenDeck={openDeck}
          onNewDeck={newDeck}
          onSelectSlide={(n) => setLibSel(n)}
        />

        {/* stage */}
        <section className="grid min-h-0 min-w-0 grid-rows-[28px_minmax(0,1fr)_auto]">
          <div className="sl-stage-head">
            {stageName ? (
              <span>
                {stageInDeck && (
                  <span style={{ color: "var(--sl-dim)" }}>
                    {activeIdx + 1}/{deck?.slides.length} ·{" "}
                  </span>
                )}
                {!stageInDeck && <span style={{ color: "var(--sl-dim)" }}>library · </span>}
                <b style={{ color: "var(--sl-fg)", fontWeight: 500 }}>{stageName}</b>
              </span>
            ) : (
              <span>stage</span>
            )}
            <span className="flex-1" />
            {libSel && deck && (
              <button
                className="sl-btn !h-5 !px-2 !text-[10.5px]"
                onClick={() => insertAt(libSel, deck.slides.length)}
              >
                + add to deck
              </button>
            )}
            {stageName && (
              <span className={cn("sl-sync", rawDirty && "sl-sync-mod")}>
                <span className="sl-sync-dot" />
                {rawDirty ? "modified · unsaved" : "synced"}
              </span>
            )}
            <button
              className={cn("sl-btn sl-btn-ghost !h-5 !px-2 !text-[10.5px]", sourceOpen && "sl-btn-active")}
              onClick={() => setSourceOpen((s) => !s)}
            >
              {sourceOpen ? "▾" : "▸"} source
            </button>
          </div>

          <div className="sl-viewport">
            {stageName ? (
              <div className="sl-slide-shell">
                {stageInDeck && (
                  <span className="sl-slide-num">
                    {String(activeIdx + 1).padStart(2, "0")}
                  </span>
                )}
                <SlideFrame
                  eager
                  key={`${stageName}·${v}·${flavor}`}
                  src={slidePreviewUrl(stageName, previewFlavor) + `&v=${v}`}
                  className="w-full"
                />
              </div>
            ) : (
              <div className="max-w-md text-center text-xs" style={{ color: "var(--sl-muted)" }}>
                <div className="sl-label mb-2">compose</div>
                open a deck from the browser (click a ▣), start a{" "}
                <button className="underline" onClick={newDeck}>
                  new deck
                </button>
                , or click any slide to inspect it.
                <div className="mt-3" style={{ color: "var(--sl-dim)" }}>
                  ←/→ navigate · ⌘S save · ⌘B build · ⌘/ source
                </div>
              </div>
            )}
          </div>

          {/* source drawer */}
          <div
            className="sl-source"
            style={{ height: sourceOpen && stageName ? 280 : 0, transition: "height 160ms ease" }}
          >
            <div className="sl-source-head">
              <span className="sl-label">source</span>
              <span>
                <b style={{ color: "var(--sl-fg)", fontWeight: 500 }}>
                  {detail.data?.relative_path ?? ""}
                </b>
              </span>
              <span className="flex-1" />
              <span style={{ color: rawDirty ? "var(--sl-warn)" : "var(--sl-primary)" }}>
                {rawDirty ? "⇣ unsaved · ⌘S to save" : "⇅ file is the truth · live"}
              </span>
              <button
                className="sl-btn !h-5 !px-2 !text-[10.5px]"
                disabled={!rawDirty || saveSlide.isPending}
                onClick={() => saveSlide.mutate()}
              >
                {saveSlide.isPending ? "saving…" : "save"}
              </button>
            </div>
            <CodeArea
              value={raw}
              onChange={(val) => {
                setRaw(val);
                setRawDirty(val !== (detail.data?.raw ?? ""));
              }}
              onSave={() => rawDirty && saveSlide.mutate()}
            />
          </div>
        </section>

        {/* side: inspector + activity */}
        <aside
          className="flex min-h-0 flex-col border-l"
          style={{ borderColor: "var(--sl-border)", background: "var(--sl-panel)" }}
        >
          <div className="sl-side-head">
            <span className="sl-label">inspector</span>
            <span className="sl-microlabel">{stageName ? baseName(stageName) : "—"}</span>
          </div>
          {stageName && detail.data ? (
            <div className="space-y-2.5 px-3 py-2.5">
              <div className="sl-field">
                <span className="sl-field-label">title</span>
                <input
                  key={stageName + "t" + v}
                  className="sl-input"
                  defaultValue={(meta.title as string) ?? ""}
                  onBlur={(e) => {
                    const t = e.target.value;
                    if (t !== ((meta.title as string) ?? "")) saveMeta.mutate({ title: t || null });
                  }}
                />
              </div>
              <div className="grid grid-cols-2 gap-2">
                <div className="sl-field">
                  <span className="sl-field-label">layout</span>
                  <select
                    key={stageName + "l" + v}
                    className="sl-input"
                    defaultValue={(meta.layout as string) ?? "default"}
                    onChange={(e) => saveMeta.mutate({ layout: e.target.value })}
                  >
                    {layouts.data?.map((l) => (
                      <option key={l.name} value={l.name}>
                        {l.name}
                      </option>
                    ))}
                  </select>
                </div>
                <div className="sl-field">
                  <span className="sl-field-label">tags</span>
                  <input
                    key={stageName + "g" + v}
                    className="sl-input"
                    defaultValue={slideTags(detail.data as unknown as SlideSummary).join(", ")}
                    placeholder="a, b"
                    onBlur={(e) => {
                      const tags = e.target.value.split(",").map((t) => t.trim()).filter(Boolean);
                      saveMeta.mutate({ tags });
                    }}
                  />
                </div>
              </div>
            </div>
          ) : (
            <div className="px-3 py-2.5 text-[11px]" style={{ color: "var(--sl-dim)" }}>
              select a slide
            </div>
          )}

          <div className="sl-side-head border-t" style={{ borderColor: "var(--sl-border)" }}>
            <span className="sl-label">activity</span>
            <span className="sl-microlabel">agent · not connected</span>
          </div>
          <div className="sl-log min-h-0 flex-1">
            {log.map((m, i) => (
              <div key={i} className={cn("sl-msg", `sl-msg-${m.kind}`)}>
                <div className="sl-msg-head">
                  <span>{m.kind}</span>
                  <span>·</span>
                  <span>{m.ts}</span>
                </div>
                <div className="sl-msg-body">{m.text}</div>
                {m.ref && (
                  <button className="sl-msg-ref" onClick={() => setLibSel(m.ref!)}>
                    ◆ {baseName(m.ref)}
                  </button>
                )}
              </div>
            ))}
          </div>
        </aside>
      </div>

      {/* timeline strip */}
      <section className="sl-strip min-w-0">
        <div className="sl-strip-head">
          <span className="sl-label">timeline</span>
          {deck ? (
            <span>
              {deck.slides.length} slides · ~{est} min
            </span>
          ) : (
            <span>no deck open</span>
          )}
          <span className="ml-auto" style={{ color: "var(--sl-dim)" }}>
            drag slides from the browser · right-click a slot for actions
          </span>
        </div>
        <div className="sl-strip-track">
          {deck ? (
            <>
              <div
                className={cn("sl-insert", dragOverIdx === 0 && "sl-insert-active")}
                onDragOver={(e) => {
                  e.preventDefault();
                  setDragOverIdx(0);
                }}
                onDragLeave={() => setDragOverIdx((i) => (i === 0 ? null : i))}
                onDrop={(e) => dropAt(0, e)}
              >
                ▸
              </div>
              {deck.slides.map((name, i) => (
                <div key={`${name}-${i}`} className="contents">
                  <div
                    className={cn("sl-slot", !libSel && activeIdx === i && "sl-slot-active")}
                    draggable
                    onDragStart={(e) => {
                      setDragIdx(i);
                      e.dataTransfer.effectAllowed = "move";
                      e.dataTransfer.setData("text/plain", "move");
                    }}
                    onDragEnd={() => {
                      setDragIdx(null);
                      setDragOverIdx(null);
                    }}
                    onClick={() => {
                      setLibSel(null);
                      setActiveIdx(i);
                    }}
                    onContextMenu={(e) => {
                      e.preventDefault();
                      setCtx({ x: e.clientX, y: e.clientY, idx: i });
                    }}
                    title={name}
                  >
                    <SlideFrame
                      src={slidePreviewUrl(name, previewFlavor) + `&v=${versions[name] ?? 0}`}
                    />
                    <div className="sl-slot-cap">
                      <span>{String(i + 1).padStart(2, "0")}</span>
                      <span className="sl-slot-cap-name">{baseName(name)}</span>
                    </div>
                  </div>
                  <div
                    className={cn("sl-insert", dragOverIdx === i + 1 && "sl-insert-active")}
                    onDragOver={(e) => {
                      e.preventDefault();
                      setDragOverIdx(i + 1);
                    }}
                    onDragLeave={() => setDragOverIdx((x) => (x === i + 1 ? null : x))}
                    onDrop={(e) => dropAt(i + 1, e)}
                  >
                    ▸
                  </div>
                </div>
              ))}
              <div className="relative">
                <button className="sl-strip-add" title="new slide" onClick={() => setNewOpen((o) => !o)}>
                  +
                </button>
                {newOpen && (
                  <div className="sl-menu bottom-[80px] left-0 !min-w-[260px]">
                    <div className="sl-microlabel px-1 pb-1">new slide (library .md)</div>
                    <form
                      className="flex gap-1.5 p-1"
                      onSubmit={(e) => {
                        e.preventDefault();
                        if (newName.trim()) {
                          createSlide.mutate(newName.trim());
                          setNewName("");
                          setNewOpen(false);
                        }
                      }}
                    >
                      <input
                        className="sl-input"
                        autoFocus
                        placeholder="topic/my-slide"
                        value={newName}
                        onChange={(e) => setNewName(e.target.value)}
                      />
                      <button className="sl-btn !h-6" type="submit" disabled={!newName.trim()}>
                        create
                      </button>
                    </form>
                  </div>
                )}
              </div>
            </>
          ) : (
            <span className="px-2 text-[11px]" style={{ color: "var(--sl-dim)" }}>
              open or create a deck to start composing
            </span>
          )}
        </div>
      </section>

      {/* context menu */}
      {ctx && deck && (
        <div
          className="sl-ctx"
          style={{ left: ctx.x, top: Math.min(ctx.y, window.innerHeight - 160) }}
          onMouseLeave={() => setCtx(null)}
        >
          <button
            className="sl-menu-item w-full"
            onClick={() => {
              setLibSel(null);
              setActiveIdx(ctx.idx);
              setSourceOpen(true);
              setCtx(null);
            }}
          >
            <span>✎</span>
            <span className="text-left">edit source</span>
            <span className="sl-menu-kbd">⌘/</span>
          </button>
          <button
            className="sl-menu-item w-full"
            onClick={() => {
              duplicateAt(ctx.idx);
              setCtx(null);
            }}
          >
            <span>⧉</span>
            <span className="text-left">duplicate</span>
            <span />
          </button>
          <div className="sl-menu-sep" />
          <button
            className="sl-menu-item w-full"
            style={{ color: "var(--sl-danger)" }}
            onClick={() => {
              removeAt(ctx.idx);
              setCtx(null);
            }}
          >
            <span>✕</span>
            <span className="text-left">remove from deck</span>
            <span className="sl-menu-kbd">⌫</span>
          </button>
        </div>
      )}
    </div>
  );
}
