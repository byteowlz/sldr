import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { api } from "@/lib/api";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Skeleton } from "@/components/ui/skeleton";

export function Layouts() {
  const layouts = useQuery({ queryKey: ["layouts"], queryFn: api.layouts });
  const [selected, setSelected] = useState<string | null>(null);
  const detail = useQuery({
    queryKey: ["layout", selected],
    queryFn: () => api.layout(selected!),
    enabled: !!selected,
  });

  return (
    <div className="grid gap-6 lg:grid-cols-[300px_1fr]">
      <section className="space-y-3">
        <h2 className="text-xl font-semibold tracking-tight">
          Layouts{" "}
          <Badge variant="secondary">{layouts.data?.length ?? "…"}</Badge>
        </h2>
        <ScrollArea className="h-[70svh] rounded-lg border p-1">
          {layouts.isLoading ? (
            <div className="space-y-1 p-1">
              {Array.from({ length: 12 }).map((_, i) => (
                <Skeleton key={i} className="h-9" />
              ))}
            </div>
          ) : (
            <div className="space-y-0.5">
              {layouts.data?.map((l) => (
                <button
                  key={l.name}
                  onClick={() => setSelected(l.name)}
                  className={
                    "flex w-full items-center justify-between gap-2 rounded-md px-3 py-2 text-left text-sm transition-colors " +
                    (selected === l.name
                      ? "bg-primary text-primary-foreground"
                      : "hover:bg-accent hover:text-accent-foreground")
                  }
                >
                  <span className="truncate font-medium">{l.name}</span>
                  <span className="shrink-0 text-xs opacity-70">
                    {l.zone_count}z{l.builtin ? "" : " ·custom"}
                  </span>
                </button>
              ))}
            </div>
          )}
        </ScrollArea>
      </section>

      <section>
        {!selected ? (
          <p className="text-sm text-muted-foreground">Select a layout.</p>
        ) : detail.isLoading ? (
          <Skeleton className="h-[70svh] rounded-lg" />
        ) : detail.data ? (
          <div className="space-y-4">
            <div className="flex flex-wrap items-center gap-2">
              <h3 className="text-lg font-semibold">{detail.data.name}</h3>
              {detail.data.category && (
                <Badge variant="outline">{detail.data.category}</Badge>
              )}
              <Badge variant={detail.data.builtin ? "secondary" : "default"}>
                {detail.data.builtin ? "built-in" : "library override"}
              </Badge>
            </div>

            <Card>
              <CardHeader>
                <CardTitle className="text-sm">
                  PPTX zones ({detail.data.zones.length})
                </CardTitle>
              </CardHeader>
              <CardContent className="space-y-1 font-mono text-xs">
                {detail.data.zones.length === 0 && (
                  <span className="text-muted-foreground">
                    No zones (screenshot-only export).
                  </span>
                )}
                {detail.data.zones.map((z, i) => (
                  <div key={i} className="flex flex-wrap gap-x-3">
                    <span className="text-primary">{z.name}</span>
                    <span className="text-muted-foreground">{z.rep}</span>
                    <span>
                      x{z.x} y{z.y} w{z.w} h{z.h}
                    </span>
                  </div>
                ))}
              </CardContent>
            </Card>

            <Card>
              <CardHeader className="flex-row items-center justify-between">
                <CardTitle className="text-sm">Source</CardTitle>
                <Button variant="outline" size="sm" disabled>
                  Edit (soon)
                </Button>
              </CardHeader>
              <CardContent>
                <ScrollArea className="h-80">
                  <pre className="text-xs leading-relaxed">
                    {detail.data.source}
                  </pre>
                </ScrollArea>
              </CardContent>
            </Card>
          </div>
        ) : null}
      </section>
    </div>
  );
}
