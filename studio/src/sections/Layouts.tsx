import { useEffect, useRef, useState } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { LayoutTemplate, Search, Save, Plus, Trash2, Code, Loader2 } from "lucide-react";
import { api, layoutPreviewUrl, type Zone } from "@/lib/api";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  Rail,
  RailHead,
  RailList,
  Row,
  Stage,
  PaneHead,
  SlideFrame,
} from "../components/shell";

const REP_STYLE: Record<string, string> = {
  "placeholder-text": "border-sky-400 bg-sky-400/10 text-sky-300",
  picture: "border-emerald-400 bg-emerald-400/10 text-emerald-300",
  shape: "border-purple-400 bg-purple-400/10 text-purple-300",
  bake: "border-red-400 bg-red-400/10 text-red-300",
};

const r1 = (v: number) => Math.round(v * 10) / 10;

/** The stage: layout sample render + draggable/resizable zone overlay. */
function ZoneCanvas({
  layout,
  flavor,
  zones,
  sel,
  onSelect,
  onChange,
}: {
  layout: string;
  flavor?: string;
  zones: Zone[];
  sel: number | null;
  onSelect: (i: number) => void;
  onChange: (i: number, z: Zone) => void;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const drag = useRef<{
    idx: number;
    mode: "move" | "resize";
    px: number;
    py: number;
    z: Zone;
  } | null>(null);

  const onPointerDown = (
    e: React.PointerEvent,
    idx: number,
    mode: "move" | "resize",
  ) => {
    e.preventDefault();
    e.stopPropagation();
    onSelect(idx);
    drag.current = { idx, mode, px: e.clientX, py: e.clientY, z: { ...zones[idx] } };
    (e.currentTarget as Element).setPointerCapture(e.pointerId);
  };
  const onPointerMove = (e: React.PointerEvent) => {
    const d = drag.current;
    const el = ref.current;
    if (!d || !el) return;
    const r = el.getBoundingClientRect();
    const dx = ((e.clientX - d.px) / r.width) * 100;
    const dy = ((e.clientY - d.py) / r.height) * 100;
    const z = { ...d.z };
    if (d.mode === "move") {
      z.x = r1(Math.min(Math.max(0, d.z.x + dx), 100 - d.z.w));
      z.y = r1(Math.min(Math.max(0, d.z.y + dy), 100 - d.z.h));
    } else {
      z.w = r1(Math.min(Math.max(2, d.z.w + dx), 100 - d.z.x));
      z.h = r1(Math.min(Math.max(2, d.z.h + dy), 100 - d.z.y));
    }
    onChange(d.idx, z);
  };
  const onPointerUp = () => (drag.current = null);

  return (
    <div
      ref={ref}
      className="relative w-full select-none"
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
    >
      <SlideFrame eager src={layoutPreviewUrl(layout, flavor)} className="w-full border" />
      {zones.map((z, i) => (
        <div
          key={i}
          onPointerDown={(e) => onPointerDown(e, i, "move")}
          style={{ left: `${z.x}%`, top: `${z.y}%`, width: `${z.w}%`, height: `${z.h}%` }}
          className={cn(
            "absolute cursor-move border touch-none",
            REP_STYLE[z.rep] ?? REP_STYLE.bake,
            sel === i && "z-10 ring-2 ring-current",
          )}
        >
          <span className="absolute left-0 top-0 max-w-full truncate bg-background/70 px-1 text-[10px] leading-4">
            {z.name}
          </span>
          <div
            onPointerDown={(e) => onPointerDown(e, i, "resize")}
            className="absolute -bottom-1 -right-1 size-3 cursor-se-resize border bg-background touch-none"
          />
        </div>
      ))}
    </div>
  );
}

function NumField({
  label,
  value,
  onChange,
}: {
  label: string;
  value: number;
  onChange: (v: number) => void;
}) {
  return (
    <label className="space-y-0.5">
      <span className="text-[10px] text-muted-foreground">{label}</span>
      <Input
        type="number"
        step={0.1}
        value={value}
        onChange={(e) => onChange(parseFloat(e.target.value) || 0)}
        className="h-7 text-xs"
      />
    </label>
  );
}

export function Layouts({ flavor }: { flavor: string }) {
  const qc = useQueryClient();
  const layouts = useQuery({ queryKey: ["layouts"], queryFn: api.layouts });
  const [name, setName] = useState<string | null>(null);
  const [q, setQ] = useState("");
  const [zones, setZones] = useState<Zone[]>([]);
  const [sel, setSel] = useState<number | null>(null);
  const [dirty, setDirty] = useState(false);
  const [showSource, setShowSource] = useState(false);

  const detail = useQuery({
    queryKey: ["layout", name],
    queryFn: () => api.layout(name!),
    enabled: !!name,
  });
  useEffect(() => {
    if (detail.data) {
      setZones(detail.data.zones);
      setSel(detail.data.zones.length ? 0 : null);
      setDirty(false);
      setShowSource(false);
    }
  }, [detail.data]);

  const save = useMutation({
    mutationFn: () => api.saveZones(name!, zones),
    onSuccess: () => {
      setDirty(false);
      qc.invalidateQueries({ queryKey: ["layouts"] });
      qc.invalidateQueries({ queryKey: ["layout", name] });
    },
  });

  const setZone = (i: number, z: Zone) => {
    setZones((zs) => zs.map((old, j) => (j === i ? z : old)));
    setDirty(true);
  };
  const addZone = () => {
    setZones((zs) => [
      ...zs,
      { name: "zone", ph: "body", idx: null, rep: "placeholder-text", x: 10, y: 10, w: 40, h: 30 },
    ]);
    setSel(zones.length);
    setDirty(true);
  };
  const deleteZone = (i: number) => {
    setZones((zs) => zs.filter((_, j) => j !== i));
    setSel(null);
    setDirty(true);
  };

  const previewFlavor = flavor === "default" ? undefined : flavor;
  const ql = q.toLowerCase();
  const z = sel !== null ? zones[sel] : null;

  return (
    <div className="grid h-full min-h-0 grid-cols-[230px_minmax(0,1fr)_270px]">
      <Rail>
        <RailHead>
          <div className="relative flex-1">
            <Search className="pointer-events-none absolute left-2 top-1/2 size-3.5 -translate-y-1/2 text-muted-foreground" />
            <Input value={q} onChange={(e) => setQ(e.target.value)} placeholder="Filter layouts…" className="h-7 pl-7 text-xs" />
          </div>
        </RailHead>
        <RailList>
          {layouts.data
            ?.filter((l) => l.name.toLowerCase().includes(ql))
            .map((l) => (
              <Row
                key={l.name}
                icon={<LayoutTemplate className="size-3.5" />}
                name={l.name}
                sub={l.builtin ? undefined : "·custom"}
                meta={`${l.zone_count}z`}
                active={name === l.name}
                onClick={() => setName(l.name)}
              />
            ))}
        </RailList>
      </Rail>

      <Stage className="border-r">
        <PaneHead title={name ? `${name} — drag zones, corner to resize` : "Zone editor"}>
          {name && (
            <Button
              variant={showSource ? "default" : "ghost"}
              size="icon"
              className="size-6"
              title="Toggle source"
              onClick={() => setShowSource((s) => !s)}
            >
              <Code className="size-3.5" />
            </Button>
          )}
        </PaneHead>
        <div className="min-h-0 flex-1 overflow-auto p-4">
          {!name ? (
            <p className="text-xs text-muted-foreground">Select a layout.</p>
          ) : showSource ? (
            <pre className="text-[11px] leading-relaxed">{detail.data?.source}</pre>
          ) : (
            <ZoneCanvas
              layout={name}
              flavor={previewFlavor}
              zones={zones}
              sel={sel}
              onSelect={setSel}
              onChange={setZone}
            />
          )}
        </div>
      </Stage>

      {/* Inspector */}
      <aside className="flex min-h-0 flex-col bg-card">
        <PaneHead title="Zones">
          <Button variant="ghost" size="icon" className="size-6" title="Add zone" onClick={addZone} disabled={!name}>
            <Plus className="size-3.5" />
          </Button>
          <Button size="sm" className="h-6 text-[11px]" disabled={!dirty || save.isPending} onClick={() => save.mutate()}>
            {save.isPending ? <Loader2 className="size-3 animate-spin" /> : <Save className="size-3" />}
            {dirty ? "Save" : "Saved"}
          </Button>
        </PaneHead>
        <div className="min-h-0 flex-1 overflow-y-auto">
          {zones.map((zone, i) => (
            <Row
              key={i}
              icon={
                <span className={cn("size-2 border", REP_STYLE[zone.rep] ?? "")} />
              }
              name={zone.name}
              meta={zone.rep === "placeholder-text" ? "text" : zone.rep}
              active={sel === i}
              onClick={() => setSel(i)}
            />
          ))}
          {name && zones.length === 0 && (
            <p className="px-3 py-2 text-[11px] text-muted-foreground">
              No zones — add one to make this layout PPTX-exportable.
            </p>
          )}

          {z && sel !== null && (
            <div className="space-y-2 border-t px-3 py-3">
              <div className="grid grid-cols-2 gap-2">
                <label className="space-y-0.5">
                  <span className="text-[10px] text-muted-foreground">name</span>
                  <Input value={z.name} onChange={(e) => setZone(sel, { ...z, name: e.target.value })} className="h-7 text-xs" />
                </label>
                <label className="space-y-0.5">
                  <span className="text-[10px] text-muted-foreground">rep</span>
                  <Select value={z.rep} onValueChange={(v) => setZone(sel, { ...z, rep: v })}>
                    <SelectTrigger className="h-7 w-full text-xs" size="sm">
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      {Object.keys(REP_STYLE).map((r) => (
                        <SelectItem key={r} value={r}>{r}</SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                </label>
                <label className="space-y-0.5">
                  <span className="text-[10px] text-muted-foreground">ph</span>
                  <Input
                    value={z.ph ?? ""}
                    placeholder="title / body / —"
                    onChange={(e) => setZone(sel, { ...z, ph: e.target.value || null })}
                    className="h-7 text-xs"
                  />
                </label>
                <label className="space-y-0.5">
                  <span className="text-[10px] text-muted-foreground">idx</span>
                  <Input
                    type="number"
                    value={z.idx ?? ""}
                    placeholder="—"
                    onChange={(e) =>
                      setZone(sel, { ...z, idx: e.target.value === "" ? null : parseInt(e.target.value) })
                    }
                    className="h-7 text-xs"
                  />
                </label>
                <NumField label="x %" value={z.x} onChange={(v) => setZone(sel, { ...z, x: v })} />
                <NumField label="y %" value={z.y} onChange={(v) => setZone(sel, { ...z, y: v })} />
                <NumField label="w %" value={z.w} onChange={(v) => setZone(sel, { ...z, w: v })} />
                <NumField label="h %" value={z.h} onChange={(v) => setZone(sel, { ...z, h: v })} />
              </div>
              <Button variant="outline" size="sm" className="h-7 w-full text-destructive" onClick={() => deleteZone(sel)}>
                <Trash2 className="size-3.5" /> Delete zone
              </Button>
            </div>
          )}

          {detail.data && (
            <p className="px-3 py-2 text-[10px] text-muted-foreground">
              {detail.data.builtin
                ? "Built-in — saving zones writes a library override."
                : "Library layout."}{" "}
              Zones are the PPTX export contract (% of the slide box).
            </p>
          )}
        </div>
      </aside>
    </div>
  );
}
