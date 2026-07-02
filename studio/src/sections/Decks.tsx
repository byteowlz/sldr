import { useState } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import {
  Hammer,
  ListVideo,
  Search,
  Loader2,
  Plus,
  X,
  ChevronLeft,
  ChevronRight,
  ExternalLink,
  Save,
  FilePlus2,
} from "lucide-react";
import {
  api,
  slidePreviewUrl,
  deckPreviewUrl,
  type Playlist,
} from "@/lib/api";
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
  RailLabel,
  Row,
  Stage,
  PaneHead,
  SlideFrame,
} from "../components/shell";

type Deck = Playlist & { isNew?: boolean };
/** What the stage is showing: a deck position or a library slide. */
type Sel = { name: string; idx: number | null };

export function Decks({ flavor }: { flavor: string }) {
  const qc = useQueryClient();
  const slides = useQuery({ queryKey: ["slides"], queryFn: api.slides });
  const playlists = useQuery({ queryKey: ["playlists"], queryFn: api.playlists });
  const flavors = useQuery({ queryKey: ["flavors"], queryFn: api.flavors });

  const [deck, setDeck] = useState<Deck | null>(null);
  const [dirty, setDirty] = useState(false);
  const [sel, setSel] = useState<Sel | null>(null);
  const [q, setQ] = useState("");
  const [msg, setMsg] = useState<{ ok: boolean; text: string } | null>(null);

  const previewFlavor = deck?.flavor || (flavor === "default" ? undefined : flavor);

  const openDeck = (p: Playlist) => {
    setDeck({ ...p });
    setDirty(false);
    setSel(p.slides.length ? { name: p.slides[0], idx: 0 } : null);
    setMsg(null);
  };
  const newDeck = () => {
    setDeck({ name: "", slides: [], isNew: true });
    setDirty(false);
    setSel(null);
    setMsg(null);
  };
  const mutateDeck = (fn: (d: Deck) => Deck) => {
    setDeck((d) => (d ? fn(d) : d));
    setDirty(true);
  };
  const addSlide = (name: string) => {
    if (!deck) return;
    mutateDeck((d) => ({ ...d, slides: [...d.slides, name] }));
    setSel({ name, idx: deck.slides.length });
  };
  const removeAt = (idx: number) => {
    mutateDeck((d) => ({ ...d, slides: d.slides.filter((_, i) => i !== idx) }));
    setSel(null);
  };
  const move = (from: number, to: number) => {
    if (!deck || to < 0 || to >= deck.slides.length) return;
    mutateDeck((d) => {
      const s = [...d.slides];
      const [x] = s.splice(from, 1);
      s.splice(to, 0, x);
      return { ...d, slides: s };
    });
    setSel({ name: deck.slides[from], idx: to });
  };

  const save = useMutation({
    mutationFn: async () => {
      if (!deck) throw new Error("no deck");
      if (!deck.name.trim()) throw new Error("Deck needs a name");
      const body: Playlist = {
        name: deck.name.trim(),
        title: deck.title ?? null,
        description: deck.description ?? null,
        slides: deck.slides,
        flavor: deck.flavor ?? null,
        render: deck.render,
      };
      if (deck.isNew) return api.createPlaylist(body);
      return api.updatePlaylist(deck.name, body);
    },
    onSuccess: () => {
      setDirty(false);
      setDeck((d) => (d ? { ...d, isNew: false } : d));
      setMsg({ ok: true, text: "Saved" });
      qc.invalidateQueries({ queryKey: ["playlists"] });
    },
    onError: (e) => setMsg({ ok: false, text: (e as Error).message }),
  });

  const build = useMutation({
    mutationFn: () => api.build(deck!.name),
    onSuccess: (r) => setMsg({ ok: true, text: `Built → ${r.html_path}` }),
    onError: (e) => setMsg({ ok: false, text: (e as Error).message }),
  });

  const [drag, setDrag] = useState<number | null>(null);
  const ql = q.toLowerCase();
  const libSlides = slides.data?.filter((s) => s.name.toLowerCase().includes(ql));

  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="grid min-h-0 flex-1 grid-cols-[250px_minmax(0,1fr)_280px]">
        {/* Browser */}
        <Rail>
          <RailHead>
            <div className="relative flex-1">
              <Search className="pointer-events-none absolute left-2 top-1/2 size-3.5 -translate-y-1/2 text-muted-foreground" />
              <Input
                value={q}
                onChange={(e) => setQ(e.target.value)}
                placeholder="Filter…"
                className="h-7 pl-7 text-xs"
              />
            </div>
            <Button variant="ghost" size="icon" className="size-7" title="New deck" onClick={newDeck}>
              <FilePlus2 className="size-4" />
            </Button>
          </RailHead>
          <RailList>
            <RailLabel>Playlists</RailLabel>
            {playlists.data
              ?.filter((p) => p.name.toLowerCase().includes(ql))
              .map((p) => (
                <Row
                  key={p.name}
                  icon={<ListVideo className="size-3.5" />}
                  name={p.name}
                  meta={p.slides.length}
                  active={deck?.name === p.name}
                  onClick={() => openDeck(p)}
                />
              ))}
            <RailLabel>Library — click to view, + to add</RailLabel>
            {libSlides?.map((s) => (
              <Row
                key={s.name}
                name={s.name}
                active={sel?.idx === null && sel?.name === s.name}
                onClick={() => setSel({ name: s.name, idx: null })}
                right={
                  deck ? (
                    <button
                      title="Add to deck"
                      className="rounded p-0.5 text-muted-foreground opacity-0 hover:bg-primary/20 hover:text-primary group-hover:opacity-100"
                      onClick={(e) => {
                        e.stopPropagation();
                        addSlide(s.name);
                      }}
                    >
                      <Plus className="size-3.5" />
                    </button>
                  ) : undefined
                }
              />
            ))}
          </RailList>
        </Rail>

        {/* Stage */}
        <Stage className="border-r">
          <PaneHead
            title={
              sel
                ? sel.idx !== null
                  ? `${sel.idx + 1} / ${deck?.slides.length} · ${sel.name}`
                  : sel.name
                : "Stage"
            }
          >
            {sel && sel.idx === null && deck && (
              <Button size="sm" variant="outline" className="h-6 text-[11px]" onClick={() => addSlide(sel.name)}>
                <Plus className="size-3" /> Add to deck
              </Button>
            )}
            {sel && sel.idx !== null && (
              <>
                <Button variant="ghost" size="icon" className="size-6" title="Move left" onClick={() => move(sel.idx!, sel.idx! - 1)}>
                  <ChevronLeft className="size-3.5" />
                </Button>
                <Button variant="ghost" size="icon" className="size-6" title="Move right" onClick={() => move(sel.idx!, sel.idx! + 1)}>
                  <ChevronRight className="size-3.5" />
                </Button>
                <Button variant="ghost" size="icon" className="size-6 text-destructive" title="Remove from deck" onClick={() => removeAt(sel.idx!)}>
                  <X className="size-3.5" />
                </Button>
              </>
            )}
          </PaneHead>
          <div className="flex min-h-0 flex-1 items-center justify-center p-4">
            {sel ? (
              <SlideFrame
                eager
                src={slidePreviewUrl(sel.name, previewFlavor)}
                className="max-h-full w-full max-w-[960px] border"
              />
            ) : (
              <p className="px-8 text-center text-xs text-muted-foreground">
                Open a playlist, or click a library slide to preview it here.
              </p>
            )}
          </div>
        </Stage>

        {/* Inspector */}
        <aside className="flex min-h-0 flex-col overflow-y-auto bg-card">
          <PaneHead title="Deck" />
          {!deck ? (
            <p className="px-3 py-3 text-xs text-muted-foreground">
              Open a playlist from the left, or create a new deck.
            </p>
          ) : (
            <div className="space-y-3 px-3 py-3">
              <label className="block space-y-1">
                <span className="text-[11px] text-muted-foreground">Name</span>
                <Input
                  value={deck.name}
                  disabled={!deck.isNew}
                  placeholder="my-deck"
                  onChange={(e) => mutateDeck((d) => ({ ...d, name: e.target.value }))}
                  className="h-7 text-xs"
                />
              </label>
              <label className="block space-y-1">
                <span className="text-[11px] text-muted-foreground">Flavor</span>
                <Select
                  value={deck.flavor ?? "__default__"}
                  onValueChange={(v) =>
                    mutateDeck((d) => ({ ...d, flavor: v === "__default__" ? null : v }))
                  }
                >
                  <SelectTrigger className="h-7 w-full text-xs" size="sm">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="__default__">default</SelectItem>
                    {flavors.data?.map((f) => (
                      <SelectItem key={f.name} value={f.name}>
                        {f.display_name || f.name}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </label>
              <div className="text-[11px] text-muted-foreground">
                {deck.slides.length} slides{dirty ? " · unsaved changes" : ""}
              </div>

              <div className="flex flex-col gap-1.5 pt-1">
                <Button size="sm" className="h-7" disabled={!dirty || save.isPending} onClick={() => save.mutate()}>
                  {save.isPending ? <Loader2 className="size-3.5 animate-spin" /> : <Save className="size-3.5" />}
                  {dirty ? "Save deck" : "Saved"}
                </Button>
                <Button
                  size="sm"
                  variant="outline"
                  className="h-7"
                  disabled={dirty || deck.isNew}
                  title={dirty ? "Save first" : "Open the presenter in a new tab"}
                  onClick={() => window.open(deckPreviewUrl(deck.name, deck.flavor ?? undefined), "_blank")}
                >
                  <ExternalLink className="size-3.5" /> Present
                </Button>
                <Button
                  size="sm"
                  variant="outline"
                  className="h-7"
                  disabled={dirty || deck.isNew || build.isPending}
                  title={dirty ? "Save first" : "Write the deck to the output dir"}
                  onClick={() => build.mutate()}
                >
                  {build.isPending ? <Loader2 className="size-3.5 animate-spin" /> : <Hammer className="size-3.5" />}
                  Build
                </Button>
              </div>

              {msg && (
                <p className={cn("break-all text-[11px]", msg.ok ? "text-primary" : "text-destructive")}>
                  {msg.text}
                </p>
              )}
            </div>
          )}
        </aside>
      </div>

      {/* Filmstrip */}
      <div className="h-[130px] shrink-0 border-t bg-card">
        {!deck ? (
          <div className="flex h-full items-center justify-center text-[11px] text-muted-foreground">
            No deck open.
          </div>
        ) : deck.slides.length === 0 ? (
          <div className="flex h-full items-center justify-center text-[11px] text-muted-foreground">
            Empty deck — add slides with the + in the library.
          </div>
        ) : (
          <div className="flex h-full items-center gap-2 overflow-x-auto px-3">
            {deck.slides.map((name, i) => (
              <div
                key={`${name}-${i}`}
                draggable
                onDragStart={() => setDrag(i)}
                onDragOver={(e) => e.preventDefault()}
                onDrop={() => {
                  if (drag !== null && drag !== i) move(drag, i);
                  setDrag(null);
                }}
                onClick={() => setSel({ name, idx: i })}
                className={cn(
                  "group relative w-[168px] shrink-0 cursor-grab border bg-background",
                  sel?.idx === i && "border-primary ring-1 ring-primary",
                )}
              >
                <SlideFrame src={slidePreviewUrl(name, previewFlavor)} />
                <div className="flex items-center gap-1 px-1.5 py-0.5">
                  <span className="text-[9px] text-muted-foreground">{i + 1}</span>
                  <span className="truncate text-[10px]">{name.split("/").pop()}</span>
                  <button
                    title="Remove"
                    className="ml-auto text-muted-foreground opacity-0 hover:text-destructive group-hover:opacity-100"
                    onClick={(e) => {
                      e.stopPropagation();
                      removeAt(i);
                    }}
                  >
                    <X className="size-3" />
                  </button>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
