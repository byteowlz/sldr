// One slide card: the real render (in the working deck's flavor), its name,
// and badges — "in deck" on sources, "×N decks" when shared. Hovering any
// card highlights every card showing the same slide on the board.

import { cn } from "@/lib/utils";
import { useSlidePreview } from "@/lib/backend/preview";
import { SlideFrame } from "../../components/shell";
import type { Item } from "./model";

export function Card({
  item,
  index,
  flavor,
  working,
  active,
  inWorking,
  uses,
  hovered,
  dimmed,
  onHover,
  onClick,
  onDragStart,
}: {
  item: Item;
  index: number;
  flavor: string | null;
  working: boolean;
  active: boolean;
  inWorking: boolean;
  uses: number;
  hovered: boolean;
  dimmed: boolean;
  onHover: (rel: string | null) => void;
  onClick: () => void;
  onDragStart: (e: React.DragEvent) => void;
}) {
  const preview = useSlidePreview(item.rel, { flavor: flavor ?? undefined });
  const name = (item.rel ?? item.raw).replace(/\.md$/, "").split("/").pop();
  return (
    <div
      className={cn("sl-card", active && "sl-card-active", hovered && "sl-card-hover", dimmed && "sl-card-dim", inWorking && !working && "sl-card-in")}
      draggable={!!item.rel}
      onDragStart={onDragStart}
      onMouseEnter={() => onHover(item.rel)}
      onMouseLeave={() => onHover(null)}
      onClick={onClick}
      title={item.rel ?? `${item.raw} — no slide by that name`}
      style={{ cursor: working ? "grab" : inWorking ? "not-allowed" : item.rel ? "copy" : "default" }}
    >
      {item.rel ? (
        <SlideFrame preview={preview.data} />
      ) : (
        <div className="sl-card-missing aspect-video">
          <span>⚠</span>
          <span>{item.raw}</span>
          <span>no slide by that name</span>
        </div>
      )}
      <div className="sl-slot-cap">
        <span>{String(index + 1).padStart(2, "0")}</span>
        <span className="sl-slot-cap-name">{name}</span>
        {uses > 1 && <span style={{ color: "var(--sl-warn)" }}>×{uses}</span>}
      </div>
      {inWorking && !working && <span className="sl-card-badge">✓ in deck</span>}
    </div>
  );
}
