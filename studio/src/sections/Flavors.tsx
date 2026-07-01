import { useEffect, useState } from "react";
import { useQuery, useMutation } from "@tanstack/react-query";
import { Palette, Save, RefreshCw, Search } from "lucide-react";
import { api, samplePreviewUrl, type Flavor } from "@/lib/api";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Rail, RailHead, RailList, RailLabel, Row, Stage, PaneHead } from "../components/shell";

type Sec = "colors" | "typography" | "spacing" | "shape" | "background";
const COLORS = [
  "background", "text", "text_dim", "accent", "primary",
  "surface", "surface2", "border", "muted",
];

function ColorRow({ label, value, onChange }: { label: string; value: string; onChange: (v: string) => void }) {
  const hex = /^#[0-9a-fA-F]{6}$/.test(value);
  return (
    <div className="flex items-center gap-2">
      <input
        type="color"
        aria-label={label}
        value={hex ? value : "#000000"}
        onChange={(e) => onChange(e.target.value)}
        className="size-6 shrink-0 border bg-transparent"
      />
      <Input value={value} onChange={(e) => onChange(e.target.value)} className="h-7 font-mono text-[11px]" />
      <span className="w-16 shrink-0 text-right text-[11px] text-muted-foreground">{label}</span>
    </div>
  );
}

function Field({ label, value, onChange, placeholder }: { label: string; value: string; onChange: (v: string) => void; placeholder?: string }) {
  return (
    <label className="space-y-1">
      <span className="text-[11px] text-muted-foreground">{label}</span>
      <Input value={value} placeholder={placeholder} onChange={(e) => onChange(e.target.value)} className="h-7 text-xs" />
    </label>
  );
}

function Group({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <section className="border-b px-3 py-2.5">
      <div className="mb-2 text-[10px] font-medium uppercase tracking-[0.12em] text-muted-foreground">{title}</div>
      {children}
    </section>
  );
}

export function Flavors() {
  const flavors = useQuery({ queryKey: ["flavors"], queryFn: api.flavors });
  const [name, setName] = useState<string | null>(null);
  const [q, setQ] = useState("");
  const [flavor, setFlavor] = useState<Flavor | null>(null);
  const [css, setCss] = useState("");
  const [dirty, setDirty] = useState(false);
  const [bust, setBust] = useState(0);

  const detail = useQuery({ queryKey: ["flavor", name], queryFn: () => api.getFlavor(name!), enabled: !!name });
  useEffect(() => {
    if (detail.data) {
      setFlavor(detail.data.flavor);
      setCss(detail.data.css ?? "");
      setDirty(false);
      setBust(Date.now());
    }
  }, [detail.data]);

  const save = useMutation({
    mutationFn: () => api.saveFlavor(name!, flavor!, css || null),
    onSuccess: () => { setDirty(false); setBust(Date.now()); },
  });

  const tok = (s: Sec, k: string) => (flavor?.[s] as Record<string, string>)?.[k] ?? "";
  const setTok = (s: Sec, k: string, v: string) => {
    setFlavor((f) => (f ? { ...f, [s]: { ...(f[s] as object), [k]: v || null } } : f));
    setDirty(true);
  };
  const setTop = (k: keyof Flavor, v: unknown) => {
    setFlavor((f) => (f ? { ...f, [k]: v } : f));
    setDirty(true);
  };

  const ql = q.toLowerCase();
  const list = flavors.data?.filter((f) => (f.display_name || f.name).toLowerCase().includes(ql));

  return (
    <div className="grid h-full min-h-0 grid-cols-[200px_minmax(0,380px)_minmax(0,1fr)]">
      {/* Flavor list */}
      <Rail>
        <RailHead>
          <div className="relative flex-1">
            <Search className="pointer-events-none absolute left-2 top-1/2 size-3.5 -translate-y-1/2 text-muted-foreground" />
            <Input value={q} onChange={(e) => setQ(e.target.value)} placeholder="Filter…" className="h-7 pl-7 text-xs" />
          </div>
        </RailHead>
        <RailList>
          <RailLabel>Flavors</RailLabel>
          {list?.map((f) => (
            <Row
              key={f.name}
              icon={<Palette className="size-3.5" />}
              name={f.display_name || f.name}
              active={name === f.name}
              onClick={() => setName(f.name)}
            />
          ))}
        </RailList>
      </Rail>

      {/* Editor */}
      <Stage className="border-r">
        <PaneHead title={name ?? "Editor"}>
          <Button size="sm" className="h-7" disabled={!flavor || !dirty || save.isPending} onClick={() => save.mutate()}>
            <Save className="size-3.5" />
            {save.isPending ? "Saving…" : dirty ? "Save" : "Saved"}
          </Button>
        </PaneHead>
        {!name ? (
          <div className="flex flex-1 items-center justify-center px-6 text-center text-xs text-muted-foreground">
            Pick a flavor to edit its <code>flavor.toml</code> / <code>flavor.css</code>.
          </div>
        ) : !flavor ? (
          <div className="flex flex-1 items-center justify-center text-xs text-muted-foreground">Loading…</div>
        ) : (
          <div className="min-h-0 flex-1 overflow-y-auto">
            <Group title="Colors">
              <div className="space-y-1.5">
                {COLORS.map((k) => (
                  <ColorRow key={k} label={k} value={tok("colors", k)} onChange={(v) => setTok("colors", k, v)} />
                ))}
              </div>
            </Group>
            <Group title="Typography">
              <div className="grid grid-cols-2 gap-2">
                <Field label="Heading font" value={tok("typography", "heading_font")} onChange={(v) => setTok("typography", "heading_font", v)} />
                <Field label="Body font" value={tok("typography", "body_font")} onChange={(v) => setTok("typography", "body_font", v)} />
                <Field label="Code font" value={tok("typography", "code_font")} onChange={(v) => setTok("typography", "code_font", v)} />
                <Field label="Type scale" placeholder="1.0" value={tok("typography", "type_scale")} onChange={(v) => setTok("typography", "type_scale", v)} />
              </div>
            </Group>
            <Group title="Spacing & shape">
              <div className="grid grid-cols-2 gap-2">
                <label className="space-y-1">
                  <span className="text-[11px] text-muted-foreground">Density</span>
                  <Select value={tok("spacing", "density") || "comfortable"} onValueChange={(v) => setTok("spacing", "density", v)}>
                    <SelectTrigger className="h-7 text-xs" size="sm"><SelectValue /></SelectTrigger>
                    <SelectContent>
                      <SelectItem value="compact">compact 0.85×</SelectItem>
                      <SelectItem value="comfortable">comfortable 1×</SelectItem>
                      <SelectItem value="spacious">spacious 1.15×</SelectItem>
                    </SelectContent>
                  </Select>
                </label>
                <Field label="Radius" placeholder="12px" value={tok("shape", "radius")} onChange={(v) => setTok("shape", "radius", v)} />
              </div>
            </Group>
            <Group title="Background & chrome">
              <div className="space-y-2">
                <div className="grid grid-cols-2 gap-2">
                  <label className="space-y-1">
                    <span className="text-[11px] text-muted-foreground">Bg type</span>
                    <Select value={tok("background", "background_type") || "color"} onValueChange={(v) => setTok("background", "background_type", v)}>
                      <SelectTrigger className="h-7 text-xs" size="sm"><SelectValue /></SelectTrigger>
                      <SelectContent>
                        {["color", "gradient", "image", "svg"].map((t) => <SelectItem key={t} value={t}>{t}</SelectItem>)}
                      </SelectContent>
                    </Select>
                  </label>
                  <Field label="Bg value" value={tok("background", "value")} onChange={(v) => setTok("background", "value", v)} />
                </div>
                <Field label="Footer" value={(flavor.footer as string) ?? ""} onChange={(v) => setTop("footer", v || null)} />
                <Field label="Chrome layouts (comma / all)" value={(flavor.chrome_layouts ?? []).join(", ")} onChange={(v) => setTop("chrome_layouts", v.split(",").map((s) => s.trim()).filter(Boolean))} />
              </div>
            </Group>
            <Group title="flavor.css (per-role sizes, etc.)">
              <textarea
                value={css}
                onChange={(e) => { setCss(e.target.value); setDirty(true); }}
                spellCheck={false}
                className="h-40 w-full resize-y border bg-background p-2 font-mono text-[11px] outline-none focus:border-ring"
              />
            </Group>
          </div>
        )}
      </Stage>

      {/* Preview */}
      <Stage>
        <PaneHead title="Preview">
          {name && (
            <Button variant="ghost" size="icon" className="size-7" onClick={() => setBust(Date.now())} aria-label="Refresh">
              <RefreshCw className="size-4" />
            </Button>
          )}
        </PaneHead>
        <div className="min-h-0 flex-1 p-3">
          {name ? (
            <div className="aspect-video w-full overflow-hidden border bg-card">
              <iframe key={bust} src={samplePreviewUrl(name, bust)} title="preview" className="size-full border-0" />
            </div>
          ) : (
            <div className="flex h-full items-center justify-center text-xs text-muted-foreground">
              Select a flavor.
            </div>
          )}
          <p className="pt-2 text-[11px] text-muted-foreground">Reflects the last save.</p>
        </div>
      </Stage>
    </div>
  );
}
