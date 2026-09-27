// Composer: the deck-building section. Orchestration only — the library
// browser, the stage, the strip and the inspector are their own modules
// under ./compose, and every question about the library goes through the
// backend (ADR-0011). The composer owns selection and keyboard routing.

import { useEffect, useRef, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { ExternalLink, Hammer, Loader2, Save } from "lucide-react";
import { cn } from "@/lib/utils";
import { useBackend } from "@/lib/backend";
import { TopBar, FlavorChip, type Chrome } from "../components/chrome";
import { Browser, baseName } from "./compose/Browser";
import { Stage } from "./compose/Stage";
import { Strip } from "./compose/Strip";
import { Inspector } from "./compose/Inspector";
import { useDeck } from "./compose/useDeck";

export function Composer({ chrome }: { chrome: Chrome }) {
  const backend = useBackend();
  const qc = useQueryClient();
  const slides = useQuery({ queryKey: ["slides"], queryFn: backend.slides });
  const playlists = useQuery({ queryKey: ["playlists"], queryFn: backend.playlists });
  const d = useDeck();

  const [libSel, setLibSel] = useState<string | null>(null);
  const [sourceOpen, setSourceOpen] = useState(true);
  const [versions, setVersions] = useState<Record<string, number>>({});
  const bump = (name: string) => setVersions((m) => ({ ...m, [name]: (m[name] ?? 0) + 1 }));

  const seeded = useRef(false);
  useEffect(() => {
    if (!seeded.current && slides.data && playlists.data) {
      seeded.current = true;
      d.pushLog("system", `studio ready · ${slides.data.length} slides · ${playlists.data.length} decks`);
    }
  }, [slides.data, playlists.data, d]);

  const stageName = libSel ?? d.deck?.slides[d.activeIdx] ?? null;
  const stageInDeck = libSel === null && !!d.deck?.slides.length;

  const createSlide = useMutation({
    mutationFn: (name: string) => backend.createSlide(name),
    onSuccess: (s) => {
      qc.invalidateQueries({ queryKey: ["slides"] });
      if (d.deck) d.insertAt(s.name, d.deck.slides.length);
      else setLibSel(s.name);
      setSourceOpen(true);
      d.pushLog("action", `created ${s.name}`, s.name);
    },
    onError: (e) => d.pushLog("system", `create failed: ${(e as Error).message}`),
  });

  useEffect(() => {
    const h = (e: KeyboardEvent) => {
      const t = e.target as HTMLElement;
      const typing = !!t.closest("input, textarea, [contenteditable]");
      const mod = e.metaKey || e.ctrlKey;
      if (mod && e.key === "b") {
        e.preventDefault();
        if (d.canBuild) d.build.mutate();
        return;
      }
      if (mod && e.key === "/") {
        e.preventDefault();
        setSourceOpen((s) => !s);
        return;
      }
      if (mod && e.key === "s" && !typing && d.dirty) {
        e.preventDefault();
        d.save.mutate();
        return;
      }
      if (typing) return;
      if (e.key === "ArrowLeft" && d.deck?.slides.length) {
        setLibSel(null);
        d.setActiveIdx((i) => Math.max(0, i - 1));
      }
      if (e.key === "ArrowRight" && d.deck?.slides.length) {
        setLibSel(null);
        d.setActiveIdx((i) => Math.min(d.deck!.slides.length - 1, i + 1));
      }
      if ((e.key === "Delete" || e.key === "Backspace") && stageInDeck) d.removeAt(d.activeIdx);
    };
    window.addEventListener("keydown", h);
    return () => window.removeEventListener("keydown", h);
  });

  const crumb = (
    <>
      <span className="sl-crumb-sep">/</span>
      {d.deck ? (
        <span
          className="sl-crumb sl-crumb-edit"
          contentEditable={!!d.deck.isNew}
          suppressContentEditableWarning
          spellCheck={false}
          title={d.deck.isNew ? "name this deck" : `deck · ${d.deck.name}`}
          onBlur={(e) => d.rename((e.target as HTMLElement).textContent?.trim() || "untitled")}
        >
          {d.deck.name}
        </span>
      ) : (
        <span className="sl-crumb">no deck open</span>
      )}
      {d.deck && (
        <span className={cn("sl-sync ml-1", d.dirty && "sl-sync-mod")}>
          <span className="sl-sync-dot" />
          {d.dirty ? "unsaved" : "saved"}
        </span>
      )}
    </>
  );

  const present = async () => {
    if (!d.deck) return;
    const p = await backend.previewDeck(d.deck.name, d.flavor ?? undefined);
    if (p.kind === "url") window.open(p.url, "_blank");
    else {
      const w = window.open("", "_blank");
      w?.document.write(p.html);
    }
  };

  const extras = (
    <>
      <button className={cn("sl-btn sl-btn-ghost", sourceOpen && "sl-btn-active")} onClick={() => setSourceOpen((s) => !s)} title="toggle source · ⌘/">
        {"{ }"} source
      </button>
      <FlavorChip value={d.flavor} onChange={d.setFlavor} />
      {d.deck && (d.dirty || d.deck.isNew) && (
        <button
          className="sl-btn"
          style={{ color: "var(--sl-warn)", borderColor: "var(--sl-warn)" }}
          onClick={() => d.save.mutate()}
          disabled={d.save.isPending}
          title="save deck · ⌘S"
        >
          {d.save.isPending ? <Loader2 className="size-3 animate-spin" /> : <Save className="size-3" />}
          save
        </button>
      )}
      {backend.canOpenExternal && (
        <button className="sl-btn" disabled={!d.canBuild} title={d.dirty ? "save first" : "open the presenter"} onClick={present}>
          present <ExternalLink className="size-3" />
        </button>
      )}
      <button className="sl-btn sl-btn-build" disabled={!d.canBuild || d.build.isPending} title={d.dirty ? "save first" : "build · ⌘B"} onClick={() => d.build.mutate()}>
        {d.build.isPending ? <Loader2 className="size-3 animate-spin" /> : <Hammer className="size-3" />}
        build
      </button>
    </>
  );

  return (
    <div className="sl-root grid h-svh w-svw max-w-svw overflow-hidden grid-rows-[40px_minmax(0,1fr)_132px]">
      <TopBar chrome={chrome} crumb={crumb} extras={extras} />
      <div className="grid min-h-0 min-w-0 grid-cols-[272px_minmax(0,1fr)_290px]">
        <Browser
          slides={slides.data ?? []}
          playlists={playlists.data ?? []}
          activeDeck={d.deck?.name ?? null}
          libSel={libSel}
          onOpenDeck={(p) => {
            if (d.open(p)) setLibSel(null);
          }}
          onNewDeck={() => {
            if (d.create()) setLibSel(null);
          }}
          onSelectSlide={setLibSel}
        />
        <Stage
          name={stageName}
          inDeck={stageInDeck}
          index={d.activeIdx}
          total={d.deck?.slides.length ?? 0}
          flavor={d.flavor}
          version={stageName ? (versions[stageName] ?? 0) : 0}
          sourceOpen={sourceOpen}
          onToggleSource={() => setSourceOpen((s) => !s)}
          onAddToDeck={libSel && d.deck ? () => {
            d.insertAt(libSel, d.deck!.slides.length);
            setLibSel(null);
          } : undefined}
          onNewDeck={() => {
            if (d.create()) setLibSel(null);
          }}
          onSaved={(n) => {
            bump(n);
            d.pushLog("action", `saved ${baseName(n)}`, n);
          }}
          onLog={(text, ref) => d.pushLog("action", text, ref)}
        />
        <Inspector
          name={stageName}
          version={stageName ? (versions[stageName] ?? 0) : 0}
          log={d.log}
          onSaved={(n) => {
            bump(n);
            d.pushLog("action", `metadata updated · ${baseName(n)}`);
          }}
          onJump={setLibSel}
        />
      </div>
      <Strip
        deck={d.deck}
        activeIdx={d.activeIdx}
        libSel={libSel}
        flavor={d.flavor}
        versions={versions}
        onSelect={(i) => {
          setLibSel(null);
          d.setActiveIdx(i);
        }}
        onInsert={(name, idx) => {
          d.insertAt(name, idx);
          setLibSel(null);
        }}
        onReorder={d.reorder}
        onRemove={d.removeAt}
        onDuplicate={d.duplicateAt}
        onEditSource={(i) => {
          setLibSel(null);
          d.setActiveIdx(i);
          setSourceOpen(true);
        }}
        onCreateSlide={(n) => createSlide.mutate(n)}
      />
    </div>
  );
}
