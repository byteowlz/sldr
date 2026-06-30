import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { api } from "@/lib/api";
import { Card, Muted, Spinner } from "@/components/ui";

export function Layouts() {
  const layouts = useQuery({ queryKey: ["layouts"], queryFn: api.layouts });
  const [selected, setSelected] = useState<string | null>(null);
  const detail = useQuery({
    queryKey: ["layout", selected],
    queryFn: () => api.layout(selected!),
    enabled: !!selected,
  });

  return (
    <div className="grid gap-4 lg:grid-cols-[280px_1fr]">
      <section className="space-y-2">
        <h2 className="text-lg font-semibold">
          Layouts <Muted>({layouts.data?.length ?? "…"})</Muted>
        </h2>
        {layouts.isLoading ? (
          <Spinner />
        ) : (
          <div className="space-y-1">
            {layouts.data?.map((l) => (
              <button
                key={l.name}
                onClick={() => setSelected(l.name)}
                className={
                  "w-full rounded-md px-3 py-2 text-left text-sm transition-colors " +
                  (selected === l.name
                    ? "bg-[var(--color-primary)] text-[var(--color-primary-fg)]"
                    : "hover:bg-[var(--color-card)]")
                }
              >
                <span className="font-medium">{l.name}</span>{" "}
                <span className="opacity-70">
                  {l.category} · {l.zone_count} zones
                  {l.builtin ? "" : " · custom"}
                </span>
              </button>
            ))}
          </div>
        )}
      </section>

      <section>
        {!selected ? (
          <Muted>Select a layout.</Muted>
        ) : detail.isLoading ? (
          <Spinner />
        ) : detail.data ? (
          <div className="space-y-3">
            <div className="flex items-baseline gap-2">
              <h3 className="text-lg font-semibold">{detail.data.name}</h3>
              <Muted>
                {detail.data.builtin ? "built-in" : "library override"}
              </Muted>
            </div>
            <Card>
              <div className="mb-2 text-sm font-medium">
                PPTX zones ({detail.data.zones.length})
              </div>
              <div className="space-y-1 text-xs">
                {detail.data.zones.map((z, i) => (
                  <div key={i} className="font-mono">
                    {z.name} · {z.rep} · x{z.x} y{z.y} w{z.w} h{z.h}
                  </div>
                ))}
              </div>
            </Card>
            <Card>
              <div className="mb-2 text-sm font-medium">Source</div>
              <pre className="overflow-auto text-xs leading-relaxed">
                {detail.data.source}
              </pre>
            </Card>
          </div>
        ) : null}
      </section>
    </div>
  );
}
