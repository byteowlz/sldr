import { useEffect, useRef, useState, type ReactNode } from "react";
import { Moon, Sun, Lock, Presentation } from "lucide-react";
import { useQuery } from "@tanstack/react-query";
import { api, clearToken } from "@/lib/api";
import { cn } from "@/lib/utils";

export type SectionId = "compose" | "flavors" | "layouts";
export interface Chrome {
  section: SectionId;
  setSection: (s: SectionId) => void;
  dark: boolean;
  setDark: (d: boolean) => void;
  onLock: () => void;
}

export function useDark() {
  const [dark, setDark] = useState(
    () => localStorage.getItem("sldr:theme") !== "light",
  );
  useEffect(() => {
    document.documentElement.classList.toggle("dark", dark);
    localStorage.setItem("sldr:theme", dark ? "dark" : "light");
  }, [dark]);
  return [dark, setDark] as const;
}

export function lockSession() {
  clearToken();
  location.reload();
}

/** The single 40px bar — logo/crumb left, section tabs, per-section extras,
 * theme + lock right. Each section renders it with its own crumb + extras. */
export function TopBar({
  chrome,
  crumb,
  extras,
}: {
  chrome: Chrome;
  crumb?: ReactNode;
  extras?: ReactNode;
}) {
  return (
    <header className="sl-bar h-10">
      <span className="sl-logo">
        <Presentation className="size-4" style={{ color: "var(--sl-primary)" }} />
        sldr
      </span>
      <nav className="sl-tabs">
        {(["compose", "flavors", "layouts"] as const).map((s) => (
          <button
            key={s}
            className={cn("sl-tab", chrome.section === s && "sl-tab-active")}
            onClick={() => chrome.setSection(s)}
          >
            {s}
          </button>
        ))}
      </nav>
      {crumb}
      <span className="flex-1" />
      {extras}
      <button
        className="sl-btn sl-btn-ghost"
        onClick={() => chrome.setDark(!chrome.dark)}
        title="theme"
      >
        {chrome.dark ? <Sun className="size-3.5" /> : <Moon className="size-3.5" />}
      </button>
      <button className="sl-btn sl-btn-ghost" onClick={chrome.onLock} title="lock">
        <Lock className="size-3.5" />
      </button>
    </header>
  );
}

/** Flavor chip + dropdown with real color swatches (from /flavors). */
export function FlavorChip({
  value,
  onChange,
  allowDefault = true,
}: {
  value: string | null;
  onChange: (f: string | null) => void;
  allowDefault?: boolean;
}) {
  const flavors = useQuery({ queryKey: ["flavors"], queryFn: api.flavors });
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!open) return;
    const close = (e: MouseEvent) => {
      if (!ref.current?.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener("mousedown", close);
    return () => document.removeEventListener("mousedown", close);
  }, [open]);

  const active = flavors.data?.find((f) => f.name === value);
  const swatch = (c?: Record<string, string | null | undefined>) =>
    c?.primary || c?.accent || "var(--sl-border-strong)";

  return (
    <div ref={ref} className="relative">
      <button className="sl-btn" onClick={() => setOpen((o) => !o)} title="flavor">
        <span className="sl-swatch" style={{ background: swatch(active?.colors) }} />
        flavor · {active ? active.display_name || active.name : "default"}
        <span style={{ color: "var(--sl-dim)" }}>▾</span>
      </button>
      {open && (
        <div className="sl-menu right-0 top-[30px]">
          <div className="sl-microlabel px-2 pb-1 pt-0.5">flavors · library</div>
          {allowDefault && (
            <button
              className={cn("sl-menu-item", value === null && "sl-menu-item-active")}
              onClick={() => {
                onChange(null);
                setOpen(false);
              }}
            >
              <span className="sl-swatch" />
              <span>
                <div>default</div>
                <div className="sl-menu-desc">config default_flavor</div>
              </span>
              {value === null && <span>◆</span>}
            </button>
          )}
          {flavors.data?.map((f) => (
            <button
              key={f.name}
              className={cn("sl-menu-item", value === f.name && "sl-menu-item-active")}
              onClick={() => {
                onChange(f.name);
                setOpen(false);
              }}
            >
              <span
                className="sl-swatch"
                style={{
                  background: swatch(f.colors),
                  borderColor: f.colors?.background || undefined,
                }}
              />
              <span>
                <div>{f.display_name || f.name}</div>
                {f.description && <div className="sl-menu-desc">{f.description}</div>}
              </span>
              {value === f.name && <span>◆</span>}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
