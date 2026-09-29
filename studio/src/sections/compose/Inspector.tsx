// Right rail: the slide's frontmatter fields (title, layout, tags), its
// where-used badge, and the activity log. Metadata edits write to the
// slide file through the backend; nothing is kept here.

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { cn } from "@/lib/utils";
import { useBackend } from "@/lib/backend";
import { baseName, slideTags } from "./Browser";
import type { LogMsg } from "./useDeck";
import type { SlideSummary } from "@/lib/api";

export function Inspector({
  name,
  version,
  log,
  onSaved,
  onJump,
}: {
  name: string | null;
  version: number;
  log: LogMsg[];
  onSaved: (name: string) => void;
  onJump: (name: string) => void;
}) {
  const backend = useBackend();
  const qc = useQueryClient();
  const detail = useQuery({
    queryKey: ["slideDetail", name, version],
    queryFn: () => backend.slideDetail(name!),
    enabled: !!name,
  });
  const layouts = useQuery({ queryKey: ["layouts"], queryFn: backend.layouts });
  const usage = useQuery({
    queryKey: ["usage", "slide", name],
    queryFn: () => backend.slideUsage(name!),
    enabled: !!name,
  });

  const saveMeta = useMutation({
    mutationFn: (meta: Record<string, unknown>) =>
      backend.saveSlideMeta(name!, { ...(detail.data?.metadata ?? {}), ...meta }),
    onSuccess: (d) => {
      qc.invalidateQueries({ queryKey: ["slides"] });
      onSaved(d.name);
    },
  });
  const meta = (detail.data?.metadata ?? {}) as Record<string, unknown>;
  const decks = usage.data?.playlists ?? [];

  return (
    <aside className="flex min-h-0 flex-col border-l" style={{ borderColor: "var(--sl-border)", background: "var(--sl-panel)" }}>
      <div className="sl-side-head">
        <span className="sl-label">inspector</span>
        <span className="sl-microlabel">{name ? baseName(name) : "—"}</span>
      </div>
      {name && detail.data ? (
        <div className="space-y-2.5 px-3 py-2.5">
          <div className="sl-field">
            <span className="sl-field-label">title</span>
            <input
              key={name + "t" + version}
              className="sl-input"
              defaultValue={(meta.title as string) ?? ""}
              onBlur={(e) => {
                const t = e.target.value;
                if (t !== ((meta.title as string) ?? "")) saveMeta.mutate({ title: t || null });
              }}
            />
          </div>
          <div className="grid grid-cols-2 gap-2">
            <div className="sl-field">
              <span className="sl-field-label">layout</span>
              <select
                key={name + "l" + version}
                className="sl-input"
                defaultValue={(meta.layout as string) ?? "default"}
                onChange={(e) => saveMeta.mutate({ layout: e.target.value })}
              >
                {layouts.data?.map((l) => (
                  <option key={l.name} value={l.name}>
                    {l.name}
                  </option>
                ))}
              </select>
            </div>
            <div className="sl-field">
              <span className="sl-field-label">tags</span>
              <input
                key={name + "g" + version}
                className="sl-input"
                defaultValue={slideTags(detail.data as unknown as SlideSummary).join(", ")}
                placeholder="a, b"
                onBlur={(e) => {
                  const tags = e.target.value.split(",").map((t) => t.trim()).filter(Boolean);
                  saveMeta.mutate({ tags });
                }}
              />
            </div>
          </div>
          <div className="sl-field">
            <span className="sl-field-label">used in</span>
            <div className="text-[11px]" style={{ color: decks.length > 1 ? "var(--sl-warn)" : "var(--sl-muted)" }}>
              {usage.isLoading ? "…" : decks.length === 0 ? "no deck" : decks.map((d) => d.name).join(", ")}
              {decks.length > 1 && <span style={{ color: "var(--sl-dim)" }}> · saving changes all {decks.length}</span>}
            </div>
          </div>
        </div>
      ) : (
        <div className="px-3 py-2.5 text-[11px]" style={{ color: "var(--sl-dim)" }}>
          select a slide
        </div>
      )}

      <div className="sl-side-head border-t" style={{ borderColor: "var(--sl-border)" }}>
        <span className="sl-label">activity</span>
        <span className="sl-microlabel">agent · not connected</span>
      </div>
      <div className="sl-log min-h-0 flex-1">
        {log.map((m, i) => (
          <div key={i} className={cn("sl-msg", `sl-msg-${m.kind}`)}>
            <div className="sl-msg-head">
              <span>{m.kind}</span>
              <span>·</span>
              <span>{m.ts}</span>
            </div>
            <div className="sl-msg-body">{m.text}</div>
            {m.ref && (
              <button className="sl-msg-ref" onClick={() => onJump(m.ref!)}>
                ◆ {baseName(m.ref)}
              </button>
            )}
          </div>
        ))}
      </div>
    </aside>
  );
}
