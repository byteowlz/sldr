// One board column: a deck (the working deck when leftmost) or a search.
// Working column accepts drops between cards and reorders; source columns
// are read-only and insert by reference.

import { useState } from "react";
import { cn } from "@/lib/utils";
import { Card } from "./Card";
import type { Item } from "./model";

export function Column({
  kind,
  title,
  meta,
  items,
  working,
  dirty,
  saving,
  flavor,
  activeIdx,
  workingSet,
  usesOf,
  hovered,
  onHover,
  onCardClick,
  onDropAt,
  onMakeWorking,
  onClose,
  onSave,
  loading,
}: {
  kind: "deck" | "search";
  title: string;
  meta: string;
  items: Item[];
  working: boolean;
  dirty?: boolean;
  saving?: boolean;
  flavor: string | null;
  activeIdx: number;
  workingSet: Set<string>;
  usesOf: (rel: string | null) => number;
  hovered: string | null;
  onHover: (rel: string | null) => void;
  onCardClick: (i: number, item: Item) => void;
  onDropAt?: (idx: number, payload: string) => void;
  onMakeWorking?: () => void;
  onClose: () => void;
  onSave?: () => void;
  loading?: boolean;
}) {
  const [over, setOver] = useState<number | null>(null);
  const label = working ? "working deck" : kind;
  const gap = (idx: number) =>
    working && onDropAt ? (
      <div
        className={cn("sl-board-gap", over === idx && "sl-board-gap-on")}
        onDragOver={(e) => {
          e.preventDefault();
          setOver(idx);
        }}
        onDragLeave={() => setOver((o) => (o === idx ? null : o))}
        onDrop={(e) => {
          e.preventDefault();
          setOver(null);
          onDropAt(idx, e.dataTransfer.getData("text/plain"));
        }}
      />
    ) : (
      <div className="h-2" />
    );

  return (
    <section className={cn("sl-board-col", working && "sl-board-col-working")} aria-label={`${label} ${title}`}>
      <header className="sl-board-head">
        <div className="flex items-center gap-2">
          <span className={cn("sl-label", kind === "search" && "!text-[var(--sl-intent)]", kind === "deck" && !working && "!text-[var(--sl-slide)]")}>
            {label}
          </span>
          <span className="flex-1" />
          {onMakeWorking && (
            <button className="sl-btn !h-5 !px-1.5 !text-[9.5px]" onClick={onMakeWorking} title="make this the working deck">
              ← work on this
            </button>
          )}
          {working && dirty && onSave && (
            <button className="sl-btn !h-5 !px-1.5 !text-[9.5px]" style={{ color: "var(--sl-warn)", borderColor: "var(--sl-warn)" }} onClick={onSave} disabled={saving}>
              {saving ? "saving…" : "save ⌘S"}
            </button>
          )}
          <button className="sl-btn sl-btn-ghost !h-5 !px-1 !text-[11px]" onClick={onClose} aria-label="close column">
            ×
          </button>
        </div>
        <div className="truncate text-[13px] font-bold" style={{ color: "var(--sl-fg)" }}>
          {title}
        </div>
        <div className="sl-microlabel truncate">
          {items.length} slides · {meta}
        </div>
      </header>
      <div
        className="sl-board-body"
        onDragOver={working && onDropAt ? (e) => e.preventDefault() : undefined}
        onDrop={
          working && onDropAt
            ? (e) => {
                if (e.defaultPrevented) return;
                e.preventDefault();
                onDropAt(items.length, e.dataTransfer.getData("text/plain"));
              }
            : undefined
        }
      >
        {loading && <div className="sl-microlabel p-2">loading…</div>}
        {gap(0)}
        {items.map((item, i) => (
          <div key={`${item.raw}-${i}`}>
            <Card
              item={item}
              index={i}
              flavor={flavor}
              working={working}
              active={working && activeIdx === i}
              inWorking={!!item.rel && workingSet.has(item.rel)}
              uses={usesOf(item.rel)}
              hovered={!!hovered && item.rel === hovered}
              dimmed={!!hovered && item.rel !== hovered}
              onHover={onHover}
              onClick={() => onCardClick(i, item)}
              onDragStart={(e) => {
                e.dataTransfer.effectAllowed = working ? "move" : "copy";
                e.dataTransfer.setData("text/plain", working ? `move:${i}` : `file:${item.rel}`);
              }}
            />
            {gap(i + 1)}
          </div>
        ))}
        {!loading && items.length === 0 && (
          <div className="sl-microlabel p-3 text-center">{working ? "drop slides here" : "nothing here"}</div>
        )}
      </div>
      <footer className="sl-board-foot sl-microlabel">
        {working
          ? "drag to reorder · ⌫ removes the reference only"
          : kind === "search"
            ? "full text · same ranking as sldr search"
            : "read-only · click a slide to insert it"}
      </footer>
    </section>
  );
}
