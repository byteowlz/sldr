import { useState } from "react";
import { useQuery, useMutation } from "@tanstack/react-query";
import { api } from "@/lib/api";
import { Button, Card, Muted, Spinner } from "@/components/ui";

export function Decks() {
  const slides = useQuery({ queryKey: ["slides"], queryFn: api.slides });
  const playlists = useQuery({ queryKey: ["playlists"], queryFn: api.playlists });
  const flavors = useQuery({ queryKey: ["flavors"], queryFn: api.flavors });
  const [flavor, setFlavor] = useState<string>("");
  const [result, setResult] = useState<string | null>(null);

  const build = useMutation({
    mutationFn: (playlist: string) => api.build(playlist, flavor || undefined),
    onSuccess: (r) => setResult(`✓ Built “${r.name}” → ${r.html_path}`),
    onError: (e) => setResult(`✗ ${(e as Error).message}`),
  });

  return (
    <div className="space-y-6">
      <section className="space-y-3">
        <div className="flex items-center justify-between gap-3">
          <h2 className="text-lg font-semibold">Playlists</h2>
          <select
            className="rounded-md border border-[var(--color-border)] bg-[var(--color-bg)] px-2 py-1.5 text-sm"
            value={flavor}
            onChange={(e) => setFlavor(e.target.value)}
          >
            <option value="">default flavor</option>
            {flavors.data?.map((f) => (
              <option key={f.name} value={f.name}>
                {f.name}
              </option>
            ))}
          </select>
        </div>
        {result && (
          <Card className="text-sm break-all">{result}</Card>
        )}
        {playlists.isLoading ? (
          <Spinner />
        ) : (
          <div className="grid gap-2 sm:grid-cols-2">
            {playlists.data?.map((p) => (
              <Card key={p.name} className="flex items-center justify-between gap-3">
                <div className="min-w-0">
                  <div className="truncate font-medium">{p.name}</div>
                  <Muted>{p.slides.length} slides</Muted>
                </div>
                <Button
                  onClick={() => build.mutate(p.name)}
                  disabled={build.isPending}
                >
                  {build.isPending ? <Spinner /> : "Build"}
                </Button>
              </Card>
            ))}
            {playlists.data?.length === 0 && <Muted>No playlists yet.</Muted>}
          </div>
        )}
      </section>

      <section className="space-y-3">
        <h2 className="text-lg font-semibold">
          Slide library{" "}
          <Muted>({slides.data?.length ?? "…"})</Muted>
        </h2>
        {slides.isLoading ? (
          <Spinner />
        ) : (
          <div className="grid gap-2 sm:grid-cols-2 lg:grid-cols-3">
            {slides.data?.map((s) => (
              <Card key={s.name} className="text-sm">
                <div className="truncate font-medium">{s.name}</div>
                <Muted>{s.relative_path}</Muted>
              </Card>
            ))}
          </div>
        )}
      </section>
    </div>
  );
}
