import { useEffect, useRef, useState, type ReactNode } from "react";
import { cn } from "@/lib/utils";

/** A slide preview iframe rendered at full logical resolution (1280×720) and
 * CSS-scaled to fit its container — so slides look exactly as presented
 * instead of reflowing/cropping at thumbnail size. */
export function SlideFrame({
  src,
  className,
  interactive = false,
  eager = false,
}: {
  src: string;
  className?: string;
  interactive?: boolean;
  eager?: boolean;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const [scale, setScale] = useState(0);
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const ro = new ResizeObserver(() => setScale(el.clientWidth / 1280));
    ro.observe(el);
    return () => ro.disconnect();
  }, []);
  return (
    <div ref={ref} className={cn("relative aspect-video overflow-hidden", className)}>
      {scale > 0 && (
        <iframe
          src={src}
          title="slide"
          loading={eager ? "eager" : "lazy"}
          tabIndex={-1}
          scrolling="no"
          className={cn("origin-top-left border-0", !interactive && "pointer-events-none")}
          style={{ width: 1280, height: 720, transform: `scale(${scale})` }}
        />
      )}
    </div>
  );
}

/** A bordered left rail (browser/list panel) — the composer's `.sl-browser`. */
export function Rail({
  children,
  className,
}: {
  children: ReactNode;
  className?: string;
}) {
  return (
    <aside
      className={cn(
        "flex min-h-0 flex-col overflow-hidden border-r bg-card",
        className,
      )}
    >
      {children}
    </aside>
  );
}

export function RailHead({ children }: { children: ReactNode }) {
  return (
    <div className="flex items-center gap-1.5 border-b px-2 py-1.5">
      {children}
    </div>
  );
}

export function RailList({ children }: { children: ReactNode }) {
  return <div className="min-h-0 flex-1 overflow-y-auto py-1">{children}</div>;
}

/** A section header inside a list (uppercase micro-label). */
export function RailLabel({ children }: { children: ReactNode }) {
  return (
    <div className="px-2.5 pb-1 pt-2 text-[10px] font-medium uppercase tracking-[0.12em] text-muted-foreground">
      {children}
    </div>
  );
}

/** A dense list row — icon · name (+sub) · meta, with hover + active tint. */
export function Row({
  icon,
  name,
  sub,
  meta,
  active,
  onClick,
  right,
}: {
  icon?: ReactNode;
  name: ReactNode;
  sub?: ReactNode;
  meta?: ReactNode;
  active?: boolean;
  onClick?: () => void;
  right?: ReactNode;
}) {
  return (
    <div
      onClick={onClick}
      className={cn(
        "group grid grid-cols-[16px_minmax(0,1fr)_auto] items-center gap-2 px-2.5 py-1.5 text-xs",
        onClick && "cursor-pointer hover:bg-foreground/[0.04]",
        active && "bg-primary/10",
      )}
    >
      <span
        className={cn(
          "flex justify-center text-muted-foreground",
          active && "text-primary",
        )}
      >
        {icon}
      </span>
      <span className="truncate">
        <span className={cn(active && "font-medium text-primary")}>{name}</span>
        {sub != null && (
          <span className="ml-1 text-muted-foreground">{sub}</span>
        )}
      </span>
      {right ?? (
        meta != null && (
          <span className="text-[10px] uppercase tracking-wide text-muted-foreground">
            {meta}
          </span>
        )
      )}
    </div>
  );
}

/** The main stage — a scrollable content pane. */
export function Stage({
  children,
  className,
}: {
  children: ReactNode;
  className?: string;
}) {
  return (
    <div className={cn("flex min-h-0 flex-col overflow-hidden", className)}>
      {children}
    </div>
  );
}

/** A compact pane header bar (title left, actions right). */
export function PaneHead({
  title,
  children,
}: {
  title: ReactNode;
  children?: ReactNode;
}) {
  return (
    <div className="flex h-9 shrink-0 items-center gap-2 border-b px-3">
      <span className="text-xs font-medium text-muted-foreground">{title}</span>
      <div className="ml-auto flex items-center gap-1">{children}</div>
    </div>
  );
}
