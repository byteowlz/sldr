// The timeline strip: the deck's slide references in order, drag to
// reorder, drag from the browser to insert, right-click for actions, and
// the new-slide popover. Every thumbnail is the real render.

import { useState } from "react";
import { cn } from "@/lib/utils";
import { useSlidePreview } from "@/lib/backend/preview";
import { SlideFrame } from "../../components/shell";
import { baseName } from "./Browser";
import type { Deck } from "./useDeck";

function Slot({
  name,
  index,
  flavor,
  version,
  active,
  onClick,
  onContext,
  onDragStart,
  onDragEnd,
}: {
  name: string;
  index: number;
  flavor: string | null;
  version: number;
  active: boolean;
  onClick: () => void;
  onContext: (x: number, y: number) => void;
  onDragStart: () => void;
  onDragEnd: () => void;
}) {
  const preview = useSlidePreview(name, { flavor: flavor ?? undefined, bust: version });
  return (
    <div
      className={cn("sl-slot", active && "sl-slot-active")}
      draggable
      onDragStart={(e) => {
        onDragStart();
        e.dataTransfer.effectAllowed = "move";
        e.dataTransfer.setData("text/plain", "move");
      }}
      onDragEnd={onDragEnd}
      onClick={onClick}
      onContextMenu={(e) => {
        e.preventDefault();
        onContext(e.clientX, e.clientY);
      }}
      title={name}
    >
      <SlideFrame preview={preview.data} />
      <div className="sl-slot-cap">
        <span>{String(index + 1).padStart(2, "0")}</span>
        <span className="sl-slot-cap-name">{baseName(name)}</span>
      </div>
    </div>
  );
}

export function Strip({
  deck,
  activeIdx,
  libSel,
  flavor,
  versions,
  onSelect,
  onInsert,
  onReorder,
  onRemove,
  onDuplicate,
  onEditSource,
  onCreateSlide,
}: {
  deck: Deck | null;
  activeIdx: number;
  libSel: string | null;
  flavor: string | null;
  versions: Record<string, number>;
  onSelect: (i: number) => void;
  onInsert: (name: string, idx: number) => void;
  onReorder: (from: number, toInsert: number) => void;
  onRemove: (i: number) => void;
  onDuplicate: (i: number) => void;
  onEditSource: (i: number) => void;
  onCreateSlide: (name: string) => void;
}) {
  const [ctx, setCtx] = useState<{ x: number; y: number; idx: number } | null>(null);
  const [dragOverIdx, setDragOverIdx] = useState<number | null>(null);
  const [dragIdx, setDragIdx] = useState<number | null>(null);
  const [newOpen, setNewOpen] = useState(false);
  const [newName, setNewName] = useState("");

  const dropAt = (idx: number, e: React.DragEvent) => {
    e.preventDefault();
    const payload = e.dataTransfer.getData("text/plain");
    if (payload.startsWith("file:")) onInsert(payload.slice(5), idx);
    else if (dragIdx !== null) onReorder(dragIdx, idx);
    setDragIdx(null);
    setDragOverIdx(null);
  };
  const insertZone = (idx: number) => (
    <div
      className={cn("sl-insert", dragOverIdx === idx && "sl-insert-active")}
      onDragOver={(e) => {
        e.preventDefault();
        setDragOverIdx(idx);
      }}
      onDragLeave={() => setDragOverIdx((x) => (x === idx ? null : x))}
      onDrop={(e) => dropAt(idx, e)}
    >
      ▸
    </div>
  );
  const est = deck ? Math.round(deck.slides.length * 1.25) : 0;

  return (
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
            {insertZone(0)}
            {deck.slides.map((name, i) => (
              <div key={`${name}-${i}`} className="contents">
                <Slot
                  name={name}
                  index={i}
                  flavor={flavor}
                  version={versions[name] ?? 0}
                  active={!libSel && activeIdx === i}
                  onClick={() => onSelect(i)}
                  onContext={(x, y) => setCtx({ x, y, idx: i })}
                  onDragStart={() => setDragIdx(i)}
                  onDragEnd={() => {
                    setDragIdx(null);
                    setDragOverIdx(null);
                  }}
                />
                {insertZone(i + 1)}
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
                        onCreateSlide(newName.trim());
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

      {ctx && deck && (
        <div
          className="sl-ctx"
          style={{ left: ctx.x, top: Math.min(ctx.y, window.innerHeight - 160) }}
          onMouseLeave={() => setCtx(null)}
        >
          <button
            className="sl-menu-item w-full"
            onClick={() => {
              onEditSource(ctx.idx);
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
              onDuplicate(ctx.idx);
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
              onRemove(ctx.idx);
              setCtx(null);
            }}
          >
            <span>✕</span>
            <span className="text-left">remove from deck</span>
            <span className="sl-menu-kbd">⌫</span>
          </button>
        </div>
      )}
    </section>
  );
}
