import { useEffect, useRef, useState } from "react";
import { useQuery, useMutation } from "@tanstack/react-query";
import { Save, RefreshCw } from "lucide-react";
import { api, samplePreviewUrl, type Flavor } from "@/lib/api";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Card,
  CardContent,
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
import { Skeleton } from "@/components/ui/skeleton";

type Section = "colors" | "typography" | "spacing" | "shape" | "background";

const COLOR_KEYS = [
  "background",
  "text",
  "text_dim",
  "accent",
  "primary",
  "surface",
  "surface2",
  "border",
  "muted",
];

function ColorRow({
  label,
  value,
  onChange,
}: {
  label: string;
  value: string;
  onChange: (v: string) => void;
}) {
  const isHex = /^#[0-9a-fA-F]{6}$/.test(value);
  return (
    <div className="flex items-center gap-2">
      <input
        type="color"
        aria-label={label}
        value={isHex ? value : "#000000"}
        onChange={(e) => onChange(e.target.value)}
        className="size-8 shrink-0 rounded border bg-transparent"
      />
      <Input
        value={value}
        onChange={(e) => onChange(e.target.value)}
        className="h-8 font-mono text-xs"
      />
      <span className="w-20 shrink-0 text-right text-xs text-muted-foreground">
        {label}
      </span>
    </div>
  );
}

function Field({
  label,
  value,
  onChange,
  placeholder,
}: {
  label: string;
  value: string;
  onChange: (v: string) => void;
  placeholder?: string;
}) {
  return (
    <div className="space-y-1">
      <Label className="text-xs">{label}</Label>
      <Input
        value={value}
        placeholder={placeholder}
        onChange={(e) => onChange(e.target.value)}
        className="h-8 text-sm"
      />
    </div>
  );
}

export function Flavors() {
  const flavors = useQuery({ queryKey: ["flavors"], queryFn: api.flavors });
  const [name, setName] = useState<string | null>(null);
  const [flavor, setFlavor] = useState<Flavor | null>(null);
  const [css, setCss] = useState("");
  const [dirty, setDirty] = useState(false);
  const [bust, setBust] = useState(0);
  const previewRef = useRef<HTMLIFrameElement>(null);

  const detail = useQuery({
    queryKey: ["flavor", name],
    queryFn: () => api.getFlavor(name!),
    enabled: !!name,
  });
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
    onSuccess: () => {
      setDirty(false);
      setBust(Date.now()); // reload preview from the freshly-saved flavor
    },
  });

  const tok =
    (s: Section, k: string) => (flavor?.[s] as Record<string, string>)?.[k] ?? "";
  const setTok = (s: Section, k: string, v: string) => {
    setFlavor((f) =>
      f ? { ...f, [s]: { ...(f[s] as object), [k]: v || null } } : f,
    );
    setDirty(true);
  };
  const setTop = (k: keyof Flavor, v: unknown) => {
    setFlavor((f) => (f ? { ...f, [k]: v } : f));
    setDirty(true);
  };

  return (
    <div className="grid gap-6 xl:grid-cols-[minmax(0,460px)_1fr]">
      {/* Editor */}
      <div className="space-y-4">
        <div className="flex items-center gap-2">
          <Select value={name ?? ""} onValueChange={setName}>
            <SelectTrigger className="w-56">
              <SelectValue placeholder="Choose a flavor to edit" />
            </SelectTrigger>
            <SelectContent>
              {flavors.data?.map((f) => (
                <SelectItem key={f.name} value={f.name}>
                  {f.display_name || f.name}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          <Button
            onClick={() => save.mutate()}
            disabled={!flavor || !dirty || save.isPending}
            className="ml-auto"
          >
            <Save className="size-4" />
            {save.isPending ? "Saving…" : dirty ? "Save" : "Saved"}
          </Button>
        </div>

        {!name ? (
          <p className="text-sm text-muted-foreground">
            Pick a flavor — edits save to its <code>flavor.toml</code> /{" "}
            <code>flavor.css</code> and the preview refreshes.
          </p>
        ) : !flavor ? (
          <Skeleton className="h-96 rounded-lg" />
        ) : (
          <div className="space-y-4">
            <Card>
              <CardHeader>
                <CardTitle className="text-sm">Colors</CardTitle>
              </CardHeader>
              <CardContent className="space-y-2">
                {COLOR_KEYS.map((k) => (
                  <ColorRow
                    key={k}
                    label={k}
                    value={tok("colors", k)}
                    onChange={(v) => setTok("colors", k, v)}
                  />
                ))}
              </CardContent>
            </Card>

            <Card>
              <CardHeader>
                <CardTitle className="text-sm">Typography</CardTitle>
              </CardHeader>
              <CardContent className="grid grid-cols-2 gap-3">
                <Field label="Heading font" value={tok("typography", "heading_font")} onChange={(v) => setTok("typography", "heading_font", v)} />
                <Field label="Body font" value={tok("typography", "body_font")} onChange={(v) => setTok("typography", "body_font", v)} />
                <Field label="Code font" value={tok("typography", "code_font")} onChange={(v) => setTok("typography", "code_font", v)} />
                <Field label="Type scale" placeholder="1.0" value={tok("typography", "type_scale")} onChange={(v) => setTok("typography", "type_scale", v)} />
                <Field label="Base size" placeholder="20px" value={tok("typography", "base_size")} onChange={(v) => setTok("typography", "base_size", v)} />
              </CardContent>
            </Card>

            <Card>
              <CardHeader>
                <CardTitle className="text-sm">Spacing & shape</CardTitle>
              </CardHeader>
              <CardContent className="grid grid-cols-2 gap-3">
                <div className="space-y-1">
                  <Label className="text-xs">Density</Label>
                  <Select
                    value={tok("spacing", "density") || "comfortable"}
                    onValueChange={(v) => setTok("spacing", "density", v)}
                  >
                    <SelectTrigger className="h-8">
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem value="compact">compact (0.85×)</SelectItem>
                      <SelectItem value="comfortable">comfortable (1×)</SelectItem>
                      <SelectItem value="spacious">spacious (1.15×)</SelectItem>
                    </SelectContent>
                  </Select>
                </div>
                <Field label="Radius" placeholder="12px" value={tok("shape", "radius")} onChange={(v) => setTok("shape", "radius", v)} />
                <Field label="Content max-width" value={tok("spacing", "content_max_width")} onChange={(v) => setTok("spacing", "content_max_width", v)} />
                <Field label="Border width" value={tok("shape", "border_width")} onChange={(v) => setTok("shape", "border_width", v)} />
              </CardContent>
            </Card>

            <Card>
              <CardHeader>
                <CardTitle className="text-sm">Background & chrome</CardTitle>
              </CardHeader>
              <CardContent className="space-y-3">
                <div className="grid grid-cols-2 gap-3">
                  <div className="space-y-1">
                    <Label className="text-xs">Background type</Label>
                    <Select
                      value={tok("background", "background_type") || "color"}
                      onValueChange={(v) => setTok("background", "background_type", v)}
                    >
                      <SelectTrigger className="h-8">
                        <SelectValue />
                      </SelectTrigger>
                      <SelectContent>
                        {["color", "gradient", "image", "svg"].map((t) => (
                          <SelectItem key={t} value={t}>{t}</SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                  </div>
                  <Field label="Background value" value={tok("background", "value")} onChange={(v) => setTok("background", "value", v)} />
                </div>
                <Field label="Footer" value={(flavor.footer as string) ?? ""} onChange={(v) => setTop("footer", v || null)} />
                <Field
                  label="Chrome layouts (comma-sep, or 'all')"
                  value={(flavor.chrome_layouts ?? []).join(", ")}
                  onChange={(v) =>
                    setTop(
                      "chrome_layouts",
                      v.split(",").map((s) => s.trim()).filter(Boolean),
                    )
                  }
                />
              </CardContent>
            </Card>

            <Card>
              <CardHeader>
                <CardTitle className="text-sm">
                  flavor.css <span className="font-normal text-muted-foreground">— escape hatch (per-role sizes, e.g. <code>--sldr-table-size</code>)</span>
                </CardTitle>
              </CardHeader>
              <CardContent>
                <textarea
                  value={css}
                  onChange={(e) => {
                    setCss(e.target.value);
                    setDirty(true);
                  }}
                  spellCheck={false}
                  className="h-48 w-full resize-y rounded-md border bg-background p-3 font-mono text-xs outline-none focus:border-ring"
                />
              </CardContent>
            </Card>
          </div>
        )}
      </div>

      {/* Live preview */}
      <div className="space-y-2">
        <div className="flex items-center justify-between">
          <span className="text-sm font-medium text-muted-foreground">
            Live preview (sample deck)
          </span>
          {name && (
            <Button
              variant="ghost"
              size="icon"
              onClick={() => setBust(Date.now())}
              aria-label="Refresh preview"
            >
              <RefreshCw className="size-4" />
            </Button>
          )}
        </div>
        <div className="aspect-video w-full overflow-hidden rounded-lg border bg-card">
          {name ? (
            <iframe
              ref={previewRef}
              key={bust}
              src={samplePreviewUrl(name, bust)}
              title="preview"
              className="h-full w-full"
            />
          ) : (
            <div className="flex h-full items-center justify-center text-sm text-muted-foreground">
              Select a flavor to preview.
            </div>
          )}
        </div>
        <p className="text-xs text-muted-foreground">
          Preview reflects the last save. Save to see your edits.
        </p>
      </div>
    </div>
  );
}
