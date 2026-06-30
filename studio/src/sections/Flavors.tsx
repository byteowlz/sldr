import { useQuery } from "@tanstack/react-query";
import { api } from "@/lib/api";
import { Card, Muted, Spinner } from "@/components/ui";

export function Flavors() {
  const flavors = useQuery({ queryKey: ["flavors"], queryFn: api.flavors });
  return (
    <section className="space-y-3">
      <h2 className="text-lg font-semibold">
        Flavors <Muted>({flavors.data?.length ?? "…"})</Muted>
      </h2>
      {flavors.isLoading ? (
        <Spinner />
      ) : (
        <div className="grid gap-2 sm:grid-cols-2 lg:grid-cols-3">
          {flavors.data?.map((f) => (
            <Card key={f.name}>
              <div className="font-medium">{f.display_name || f.name}</div>
              <Muted>{f.description || f.name}</Muted>
            </Card>
          ))}
        </div>
      )}
      <Muted>Editing arrives with the flavor editor — the API is ready.</Muted>
    </section>
  );
}
