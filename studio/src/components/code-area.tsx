import { useRef } from "react";

const esc = (s: string) =>
  s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");

/** Regex highlighter for a slide file: YAML frontmatter + markdown body. */
export function highlightSlideSource(src: string): string {
  const lines = src.split("\n");
  let inFm = false;
  let inFence = false;
  return lines
    .map((line, i) => {
      let h = esc(line);
      if (/^---\s*$/.test(line) && (i === 0 || inFm)) {
        inFm = i === 0 ? true : false;
        return `<span class="tok-section">${h}</span>`;
      }
      if (inFm) {
        // single pass — never re-scan inserted markup
        return h.replace(
          /(#.*$)|("(?:[^"\\]|\\.)*")|(^\s*[A-Za-z_][\w-]*(?=\s*:))/g,
          (_m, c, s, k) =>
            c ? `<span class="tok-comment">${c}</span>`
            : s ? `<span class="tok-str">${s}</span>`
            : `<span class="tok-key">${k}</span>`,
        );
      }
      if (/^```/.test(line)) {
        inFence = !inFence;
        return `<span class="tok-punct">${h}</span>`;
      }
      if (inFence) return `<span class="tok-str">${h}</span>`;
      // markdown body — single pass, no re-scan of inserted markup
      if (/^#{1,6}\s/.test(line)) return `<span class="tok-section">${h}</span>`;
      return h.replace(
        /(&lt;!--.*?--&gt;)|(::[a-z0-9:-]+::)|(!?\[[^\]]*\]\([^)]*\))|(\*\*[^*]+\*\*)|(^\s*[-*]\s)/gi,
        (_m, cm, mk, ln, bd, bu) =>
          cm ? `<span class="tok-comment">${cm}</span>`
          : mk ? `<span class="tok-marker">${mk}</span>`
          : ln ? `<span class="tok-str">${ln}</span>`
          : bd ? `<span class="tok-key">${bd}</span>`
          : `<span class="tok-punct">${bu}</span>`,
      );
    })
    .join("\n");
}

/** Overlay code editor: a transparent textarea over a highlighted <pre> —
 * robust caret behavior, zero deps. */
export function CodeArea({
  value,
  onChange,
  onSave,
}: {
  value: string;
  onChange: (v: string) => void;
  onSave?: () => void;
}) {
  const preRef = useRef<HTMLPreElement>(null);
  return (
    <div className="sl-code">
      <pre
        ref={preRef}
        className="sl-code-pre"
        aria-hidden
        dangerouslySetInnerHTML={{ __html: highlightSlideSource(value) + "\n" }}
      />
      <textarea
        className="sl-code-ta"
        value={value}
        spellCheck={false}
        onChange={(e) => onChange(e.target.value)}
        onScroll={(e) => {
          const p = preRef.current;
          if (p) {
            p.scrollTop = e.currentTarget.scrollTop;
            p.scrollLeft = e.currentTarget.scrollLeft;
          }
        }}
        onKeyDown={(e) => {
          if ((e.metaKey || e.ctrlKey) && e.key === "s") {
            e.preventDefault();
            onSave?.();
          }
        }}
      />
    </div>
  );
}
