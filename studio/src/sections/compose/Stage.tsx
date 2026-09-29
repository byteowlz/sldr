// The stage: one slide rendered by the real compiler (an iframe of the
// backend's preview), with the source drawer under it. Text edits go to the
// markdown file; the file is the truth and the preview follows it.

import { useEffect, useRef, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { cn } from "@/lib/utils";
import { useBackend } from "@/lib/backend";
import { useSlidePreview } from "@/lib/backend/preview";
import { SlideFrame } from "../../components/shell";
import { CodeArea } from "../../components/code-area";
import { baseName } from "./Browser";
import { BackendError } from "@/lib/backend";
import { ApiError } from "@/lib/api";
import { isMediaFile, markdownFor, mediaNameFor } from "@/lib/media";

export function Stage({
  name,
  inDeck,
  index,
  total,
  flavor,
  version,
  sourceOpen,
  onToggleSource,
  onAddToDeck,
  onNewDeck,
  onSaved,
  onLog,
}: {
  name: string | null;
  inDeck: boolean;
  index: number;
  total: number;
  flavor: string | null;
  version: number;
  sourceOpen: boolean;
  onToggleSource: () => void;
  onAddToDeck?: () => void;
  onNewDeck: () => void;
  onSaved: (name: string) => void;
  onLog?: (text: string, ref?: string) => void;
}) {
  const backend = useBackend();
  const qc = useQueryClient();
  const detail = useQuery({
    queryKey: ["slideDetail", name, version],
    queryFn: () => backend.slideDetail(name!),
    enabled: !!name,
  });
  const preview = useSlidePreview(name, { flavor: flavor ?? undefined, bust: version });

  const [raw, setRaw] = useState("");
  const [rawDirty, setRawDirty] = useState(false);
  useEffect(() => {
    setRaw(detail.data?.raw ?? "");
    setRawDirty(false);
  }, [detail.data?.raw, name]);

  const save = useMutation({
    mutationFn: () => backend.saveSlideRaw(name!, raw),
    onSuccess: (d) => {
      setRawDirty(false);
      qc.invalidateQueries({ queryKey: ["slides"] });
      onSaved(d.name);
    },
  });

  // ---- paste / drop an image → stored beside the slide, reference inserted ----
  // Uses the paste event (not the async Clipboard API) so it works over
  // plain HTTP on a tailnet.
  const taRef = useRef<HTMLTextAreaElement | null>(null);
  const [dropping, setDropping] = useState(false);
  const attach = async (file: File) => {
    if (!name) return;
    let stored: { reference: string } | null = null;
    for (let attempt = 0; attempt < 5 && !stored; attempt++) {
      try {
        stored = await backend.uploadMedia(name, file, { name: mediaNameFor(name, file, attempt) });
      } catch (e) {
        const conflict =
          (e instanceof BackendError && e.code === "conflict") || (e instanceof ApiError && e.status === 409);
        if (!conflict) {
          onLog?.(`media upload failed: ${(e as Error).message}`);
          return;
        }
      }
    }
    if (!stored) return;
    const md = markdownFor(stored.reference);
    const ta = taRef.current;
    if (ta && document.activeElement === ta) {
      // Insert at the caret; the user saves with the rest of their edit.
      const at = ta.selectionStart ?? raw.length;
      const next = raw.slice(0, at) + md + raw.slice(ta.selectionEnd ?? at);
      setRaw(next);
      setRawDirty(true);
      requestAnimationFrame(() => ta.setSelectionRange(at + md.length, at + md.length));
    } else if (!rawDirty) {
      // Nothing pending: append and save straight away.
      const next = `${raw.trimEnd()}\n\n${md}\n`;
      const saved = await backend.saveSlideRaw(name, next);
      qc.invalidateQueries({ queryKey: ["slides"] });
      qc.invalidateQueries({ queryKey: ["media"] });
      onSaved(saved.name);
    } else {
      setRaw(`${raw.trimEnd()}\n\n${md}\n`);
      setRawDirty(true);
    }
    onLog?.(`added ${stored.reference} to ${baseName(name)}`, name);
  };
  useEffect(() => {
    if (!name) return;
    const onPaste = (e: ClipboardEvent) => {
      const files = [...(e.clipboardData?.files ?? [])].filter(isMediaFile);
      if (!files.length) return;
      e.preventDefault();
      files.forEach((f) => void attach(f));
    };
    window.addEventListener("paste", onPaste);
    return () => window.removeEventListener("paste", onPaste);
  });

  // ⌘S inside the source editor saves the slide.
  useEffect(() => {
    const h = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === "s" && rawDirty) {
        e.preventDefault();
        save.mutate();
      }
    };
    window.addEventListener("keydown", h);
    return () => window.removeEventListener("keydown", h);
  }, [rawDirty, save]);

  return (
    <section className="grid min-h-0 min-w-0 grid-rows-[28px_minmax(0,1fr)_auto]">
      <div className="sl-stage-head">
        {name ? (
          <span>
            {inDeck && (
              <span style={{ color: "var(--sl-dim)" }}>
                {index + 1}/{total} ·{" "}
              </span>
            )}
            {!inDeck && <span style={{ color: "var(--sl-dim)" }}>library · </span>}
            <b style={{ color: "var(--sl-fg)", fontWeight: 500 }}>{name}</b>
          </span>
        ) : (
          <span>stage</span>
        )}
        <span className="flex-1" />
        {onAddToDeck && (
          <button className="sl-btn !h-5 !px-2 !text-[10.5px]" onClick={onAddToDeck}>
            + add to deck
          </button>
        )}
        {name && (
          <span className={cn("sl-sync", rawDirty && "sl-sync-mod")}>
            <span className="sl-sync-dot" />
            {rawDirty ? "modified · unsaved" : "synced"}
          </span>
        )}
        <button
          className={cn("sl-btn sl-btn-ghost !h-5 !px-2 !text-[10.5px]", sourceOpen && "sl-btn-active")}
          onClick={onToggleSource}
        >
          {sourceOpen ? "▾" : "▸"} source
        </button>
      </div>

      <div
        className={cn("sl-viewport", dropping && "outline outline-2 -outline-offset-4 outline-[var(--sl-primary)]")}
        onDragOver={(e) => {
          if (name && [...e.dataTransfer.types].includes("Files")) {
            e.preventDefault();
            setDropping(true);
          }
        }}
        onDragLeave={() => setDropping(false)}
        onDrop={(e) => {
          const files = [...e.dataTransfer.files].filter(isMediaFile);
          setDropping(false);
          if (!name || !files.length) return;
          e.preventDefault();
          files.forEach((f) => void attach(f));
        }}
      >
        {name ? (
          <div className="sl-slide-shell">
            {inDeck && <span className="sl-slide-num">{String(index + 1).padStart(2, "0")}</span>}
            <SlideFrame eager key={`${name}·${version}·${flavor}`} preview={preview.data} className="w-full" />
          </div>
        ) : (
          <div className="max-w-md text-center text-xs" style={{ color: "var(--sl-muted)" }}>
            <div className="sl-label mb-2">compose</div>
            open a deck from the browser (click a ▣), start a{" "}
            <button className="underline" onClick={onNewDeck}>
              new deck
            </button>
            , or click any slide to inspect it.
            <div className="mt-3" style={{ color: "var(--sl-dim)" }}>
              ←/→ navigate · ⌘S save · ⌘B build · ⌘/ source · paste or drop an image onto a slide
            </div>
          </div>
        )}
      </div>

      <div className="sl-source" style={{ height: sourceOpen && name ? 280 : 0, transition: "height 160ms ease" }}>
        <div className="sl-source-head">
          <span className="sl-label">source</span>
          <span>
            <b style={{ color: "var(--sl-fg)", fontWeight: 500 }}>{detail.data?.relative_path ?? ""}</b>
          </span>
          <span className="flex-1" />
          <span style={{ color: rawDirty ? "var(--sl-warn)" : "var(--sl-primary)" }}>
            {rawDirty ? "⇣ unsaved · ⌘S to save" : "⇅ file is the truth · live"}
          </span>
          <button className="sl-btn !h-5 !px-2 !text-[10.5px]" disabled={!rawDirty || save.isPending} onClick={() => save.mutate()}>
            {save.isPending ? "saving…" : "save"}
          </button>
        </div>
        <CodeArea
          textareaRef={taRef}
          value={raw}
          onChange={(val) => {
            setRaw(val);
            setRawDirty(val !== (detail.data?.raw ?? ""));
          }}
          onSave={() => rawDirty && save.mutate()}
        />
      </div>
      {name && <span hidden data-stage={baseName(name)} />}
    </section>
  );
}
