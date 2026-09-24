// Library browser (FS / TAGS / RECENT) — the left rail of the composer.
// Pure presentation over the slide + playlist lists; selection and deck
// state live in the composer. Split out of Composer.tsx (ADR-0011).

import { useMemo, useState } from "react";
import type { Playlist, SlideSummary } from "@/lib/api";
import { cn } from "@/lib/utils";

export function Row({
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

export const slideDate = (s: SlideSummary) =>
  (s.metadata?.modified as string) || (s.metadata?.created as string) || "";
export const slideTags = (s: SlideSummary) =>
  Array.isArray(s.metadata?.tags) ? (s.metadata.tags as string[]) : [];
export const baseName = (n: string) => n.split("/").pop() ?? n;

export function Browser({
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
