import { useQuery } from "@tanstack/react-query";
import { Palette } from "lucide-react";
import { api } from "@/lib/api";
import { Badge } from "@/components/ui/badge";
import {
  Card,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";

export function Flavors() {
  const flavors = useQuery({ queryKey: ["flavors"], queryFn: api.flavors });
  return (
    <section className="space-y-4">
      <div className="flex items-center gap-2">
        <h2 className="text-xl font-semibold tracking-tight">Flavors</h2>
        <Badge variant="secondary">{flavors.data?.length ?? "…"}</Badge>
      </div>
      {flavors.isLoading ? (
        <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
          {Array.from({ length: 6 }).map((_, i) => (
            <Skeleton key={i} className="h-24 rounded-xl" />
          ))}
        </div>
      ) : (
        <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
          {flavors.data?.map((f) => (
            <Card key={f.name}>
              <CardHeader>
                <CardTitle className="flex items-center gap-2">
                  <Palette className="size-4 text-muted-foreground" />
                  {f.display_name || f.name}
                </CardTitle>
                <CardDescription>{f.description || f.name}</CardDescription>
              </CardHeader>
            </Card>
          ))}
        </div>
      )}
      <p className="text-sm text-muted-foreground">
        Visual editing arrives next — the GET/PUT flavor API is already live.
      </p>
    </section>
  );
}
