import { useState } from "react";
import { useQuery, useMutation } from "@tanstack/react-query";
import { Hammer, FileText, CheckCircle2, XCircle } from "lucide-react";
import { api } from "@/lib/api";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Badge } from "@/components/ui/badge";
import { Skeleton } from "@/components/ui/skeleton";

function CardGridSkeleton({ n = 6 }: { n?: number }) {
  return (
    <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
      {Array.from({ length: n }).map((_, i) => (
        <Skeleton key={i} className="h-20 rounded-xl" />
      ))}
    </div>
  );
}

export function Decks() {
  const slides = useQuery({ queryKey: ["slides"], queryFn: api.slides });
  const playlists = useQuery({ queryKey: ["playlists"], queryFn: api.playlists });
  const flavors = useQuery({ queryKey: ["flavors"], queryFn: api.flavors });
  const [flavor, setFlavor] = useState("default");
  const [result, setResult] = useState<{ ok: boolean; msg: string } | null>(null);

  const build = useMutation({
    mutationFn: (playlist: string) =>
      api.build(playlist, flavor === "default" ? undefined : flavor),
    onSuccess: (r) => setResult({ ok: true, msg: `Built “${r.name}” → ${r.html_path}` }),
    onError: (e) => setResult({ ok: false, msg: (e as Error).message }),
  });

  return (
    <div className="space-y-8">
      <section className="space-y-4">
        <div className="flex flex-wrap items-center justify-between gap-3">
          <div>
            <h2 className="text-xl font-semibold tracking-tight">Playlists</h2>
            <p className="text-sm text-muted-foreground">
              Build a deck — pick a flavor, hit build.
            </p>
          </div>
          <Select value={flavor} onValueChange={setFlavor}>
            <SelectTrigger className="w-48">
              <SelectValue placeholder="Flavor" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="default">default flavor</SelectItem>
              {flavors.data?.map((f) => (
                <SelectItem key={f.name} value={f.name}>
                  {f.display_name || f.name}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>

        {result && (
          <div
            className={
              "flex items-start gap-2 rounded-lg border p-3 text-sm " +
              (result.ok
                ? "border-primary/30 bg-primary/5"
                : "border-destructive/30 bg-destructive/5 text-destructive")
            }
          >
            {result.ok ? (
              <CheckCircle2 className="mt-0.5 size-4 shrink-0 text-primary" />
            ) : (
              <XCircle className="mt-0.5 size-4 shrink-0" />
            )}
            <span className="break-all">{result.msg}</span>
          </div>
        )}

        {playlists.isLoading ? (
          <CardGridSkeleton />
        ) : (
          <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
            {playlists.data?.map((p) => (
              <Card key={p.name} className="gap-3">
                <CardHeader>
                  <CardTitle className="truncate">{p.name}</CardTitle>
                  <CardDescription>{p.slides.length} slides</CardDescription>
                </CardHeader>
                <CardContent>
                  <Button
                    className="w-full"
                    onClick={() => build.mutate(p.name)}
                    disabled={build.isPending}
                  >
                    <Hammer className="size-4" />
                    {build.isPending && build.variables === p.name
                      ? "Building…"
                      : "Build"}
                  </Button>
                </CardContent>
              </Card>
            ))}
            {playlists.data?.length === 0 && (
              <p className="text-sm text-muted-foreground">No playlists yet.</p>
            )}
          </div>
        )}
      </section>

      <section className="space-y-4">
        <div className="flex items-center gap-2">
          <h2 className="text-xl font-semibold tracking-tight">Slide library</h2>
          <Badge variant="secondary">{slides.data?.length ?? "…"}</Badge>
        </div>
        {slides.isLoading ? (
          <CardGridSkeleton />
        ) : (
          <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
            {slides.data?.map((s) => (
              <Card key={s.name} className="gap-2 py-4">
                <CardHeader className="px-4">
                  <CardTitle className="flex items-center gap-2 truncate text-base">
                    <FileText className="size-4 shrink-0 text-muted-foreground" />
                    {s.name}
                  </CardTitle>
                  <CardDescription className="truncate">
                    {s.relative_path}
                  </CardDescription>
                </CardHeader>
              </Card>
            ))}
          </div>
        )}
      </section>
    </div>
  );
}
