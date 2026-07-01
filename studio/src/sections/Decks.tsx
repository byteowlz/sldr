import { useMemo, useState } from "react";
import { useQuery, useMutation } from "@tanstack/react-query";
import { Hammer, ListVideo, Library, Search, Check, X, Loader2 } from "lucide-react";
import { api, slidePreviewUrl } from "@/lib/api";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Rail, RailHead, RailList, RailLabel, Row, Stage, PaneHead } from "../components/shell";

export function Decks({ flavor }: { flavor: string }) {
  const slides = useQuery({ queryKey: ["slides"], queryFn: api.slides });
  const playlists = useQuery({ queryKey: ["playlists"], queryFn: api.playlists });
  const [sel, setSel] = useState<string>("__all__");
  const [q, setQ] = useState("");
  const [result, setResult] = useState<{ ok: boolean; msg: string } | null>(null);

  const build = useMutation({
    mutationFn: (name: string) =>
      api.build(name, flavor === "default" ? undefined : flavor),
    onSuccess: (r) => setResult({ ok: true, msg: `Built → ${r.html_path}` }),
    onError: (e) => setResult({ ok: false, msg: (e as Error).message }),
  });

  const active = playlists.data?.find((p) => p.name === sel) ?? null;
  const names = useMemo(() => {
    if (sel === "__all__") return slides.data?.map((s) => s.name) ?? [];
    return active?.slides ?? [];
  }, [sel, active, slides.data]);

  const ql = q.toLowerCase();
  const showPlaylists = playlists.data?.filter((p) => p.name.toLowerCase().includes(ql));

  return (
    <div className="grid h-full min-h-0 grid-cols-[240px_minmax(0,1fr)]">
      <Rail>
        <RailHead>
          <div className="relative flex-1">
            <Search className="pointer-events-none absolute left-2 top-1/2 size-3.5 -translate-y-1/2 text-muted-foreground" />
            <Input
              value={q}
              onChange={(e) => setQ(e.target.value)}
              placeholder="Filter playlists…"
              className="h-7 pl-7 text-xs"
            />
          </div>
        </RailHead>
        <RailList>
          <Row
            icon={<Library className="size-3.5" />}
            name="All slides"
            meta={slides.data?.length}
            active={sel === "__all__"}
            onClick={() => setSel("__all__")}
          />
          <RailLabel>Playlists</RailLabel>
          {showPlaylists?.map((p) => (
            <Row
              key={p.name}
              icon={<ListVideo className="size-3.5" />}
              name={p.name}
              meta={p.slides.length}
              active={sel === p.name}
              onClick={() => setSel(p.name)}
            />
          ))}
        </RailList>
      </Rail>

      <Stage>
        <PaneHead title={sel === "__all__" ? "All slides" : sel}>
          {result && (
            <span
              className={
                "flex items-center gap-1 truncate text-[11px] " +
                (result.ok ? "text-primary" : "text-destructive")
              }
            >
              {result.ok ? <Check className="size-3" /> : <X className="size-3" />}
              {result.msg}
            </span>
          )}
          <span className="text-[11px] text-muted-foreground">{names.length} slides</span>
          {active && (
            <Button
              size="sm"
              className="h-7"
              disabled={build.isPending}
              onClick={() => build.mutate(active.name)}
            >
              {build.isPending ? (
                <Loader2 className="size-3.5 animate-spin" />
              ) : (
                <Hammer className="size-3.5" />
              )}
              Build
            </Button>
          )}
        </PaneHead>

        <div className="min-h-0 flex-1 overflow-y-auto p-3">
          {names.length === 0 ? (
            <p className="text-xs text-muted-foreground">
              {sel === "__all__" ? "No slides." : "This playlist has no slides."}
            </p>
          ) : (
            <div className="grid grid-cols-2 gap-2 md:grid-cols-3 lg:grid-cols-4 2xl:grid-cols-5">
              {names.map((name) => (
                <figure
                  key={name}
                  className="group overflow-hidden border bg-card"
                >
                  <div className="aspect-video overflow-hidden border-b bg-muted">
                    <iframe
                      src={slidePreviewUrl(
                        name,
                        flavor === "default" ? undefined : flavor,
                      )}
                      title={name}
                      loading="lazy"
                      tabIndex={-1}
                      scrolling="no"
                      className="pointer-events-none size-full border-0"
                    />
                  </div>
                  <figcaption className="truncate px-2 py-1.5 text-[11px] text-muted-foreground group-hover:text-foreground">
                    {name}
                  </figcaption>
                </figure>
              ))}
            </div>
          )}
        </div>
      </Stage>
    </div>
  );
}
