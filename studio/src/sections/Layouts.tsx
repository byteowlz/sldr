import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { LayoutTemplate, Search } from "lucide-react";
import { api } from "@/lib/api";
import { Input } from "@/components/ui/input";
import { Rail, RailHead, RailList, Row, Stage, PaneHead } from "../components/shell";

export function Layouts() {
  const layouts = useQuery({ queryKey: ["layouts"], queryFn: api.layouts });
  const [sel, setSel] = useState<string | null>(null);
  const [q, setQ] = useState("");
  const detail = useQuery({
    queryKey: ["layout", sel],
    queryFn: () => api.layout(sel!),
    enabled: !!sel,
  });
  const ql = q.toLowerCase();
  const list = layouts.data?.filter((l) => l.name.toLowerCase().includes(ql));

  return (
    <div className="grid h-full min-h-0 grid-cols-[240px_minmax(0,1fr)]">
      <Rail>
        <RailHead>
          <div className="relative flex-1">
            <Search className="pointer-events-none absolute left-2 top-1/2 size-3.5 -translate-y-1/2 text-muted-foreground" />
            <Input
              value={q}
              onChange={(e) => setQ(e.target.value)}
              placeholder="Filter layouts…"
              className="h-7 pl-7 text-xs"
            />
          </div>
        </RailHead>
        <RailList>
          {list?.map((l) => (
            <Row
              key={l.name}
              icon={<LayoutTemplate className="size-3.5" />}
              name={l.name}
              sub={l.builtin ? undefined : "·custom"}
              meta={`${l.zone_count}z`}
              active={sel === l.name}
              onClick={() => setSel(l.name)}
            />
          ))}
        </RailList>
      </Rail>

      <Stage>
        {!sel ? (
          <div className="flex flex-1 items-center justify-center text-xs text-muted-foreground">
            Select a layout.
          </div>
        ) : !detail.data ? (
          <div className="flex flex-1 items-center justify-center text-xs text-muted-foreground">
            Loading…
          </div>
        ) : (
          <>
            <PaneHead title={detail.data.name}>
              <span className="text-[11px] text-muted-foreground">
                {detail.data.category ?? "—"} ·{" "}
                {detail.data.builtin ? "built-in" : "library override"}
              </span>
            </PaneHead>
            <div className="min-h-0 flex-1 overflow-y-auto">
              {/* Zones */}
              <div className="border-b">
                <div className="px-3 py-1.5 text-[10px] font-medium uppercase tracking-[0.12em] text-muted-foreground">
                  PPTX zones ({detail.data.zones.length})
                </div>
                {detail.data.zones.length === 0 ? (
                  <div className="px-3 pb-2 text-xs text-muted-foreground">
                    None — screenshot-only export.
                  </div>
                ) : (
                  <div className="pb-2 font-mono text-[11px]">
                    {detail.data.zones.map((z, i) => (
                      <div
                        key={i}
                        className="grid grid-cols-[1fr_5rem_1fr] gap-2 px-3 py-0.5"
                      >
                        <span className="truncate text-primary">{z.name}</span>
                        <span className="text-muted-foreground">{z.rep}</span>
                        <span className="text-muted-foreground">
                          x{z.x} y{z.y} w{z.w} h{z.h}
                        </span>
                      </div>
                    ))}
                  </div>
                )}
              </div>
              {/* Source */}
              <div className="px-3 py-1.5 text-[10px] font-medium uppercase tracking-[0.12em] text-muted-foreground">
                Source
              </div>
              <pre className="px-3 pb-4 text-[11px] leading-relaxed">
                {detail.data.source}
              </pre>
            </div>
          </>
        )}
      </Stage>
    </div>
  );
}
