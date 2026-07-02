import { useEffect, useState } from "react";
import { Presentation } from "lucide-react";
import { api, ApiError, setToken } from "@/lib/api";
import { useDark, lockSession, type Chrome, type SectionId } from "./components/chrome";
import { Composer } from "./sections/Composer";
import { Flavors } from "./sections/Flavors";
import { Layouts } from "./sections/Layouts";

function Login({ onAuthed }: { onAuthed: () => void }) {
  const [token, setTok] = useState("");
  const [err, setErr] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    setBusy(true);
    setErr(null);
    setToken(token);
    try {
      await api.slides();
      onAuthed();
    } catch (e) {
      setErr(e instanceof ApiError && e.status === 401 ? "invalid token" : String(e));
      setBusy(false);
    }
  };
  return (
    <div className="sl-root flex min-h-svh items-center justify-center p-6">
      <form
        onSubmit={submit}
        className="w-full max-w-xs space-y-3 border p-5"
        style={{ borderColor: "var(--sl-border-strong)", background: "var(--sl-panel)" }}
      >
        <div className="sl-logo">
          <Presentation className="size-4" style={{ color: "var(--sl-primary)" }} />
          sldr studio
        </div>
        <p className="text-[11px]" style={{ color: "var(--sl-muted)" }}>
          server token (SLDR_API_TOKEN)
        </p>
        <input
          className="sl-input !h-8"
          type="password"
          value={token}
          onChange={(e) => setTok(e.target.value)}
          autoFocus
        />
        {err && (
          <p className="text-[11px]" style={{ color: "var(--sl-danger)" }}>
            {err}
          </p>
        )}
        <button className="sl-btn sl-btn-build w-full justify-center" disabled={busy}>
          {busy ? "unlocking…" : "unlock"}
        </button>
      </form>
    </div>
  );
}

export default function App() {
  const [dark, setDark] = useDark();
  const [authed, setAuthed] = useState<boolean | null>(null);
  const [section, setSection] = useState<SectionId>("compose");

  useEffect(() => {
    api
      .slides()
      .then(() => setAuthed(true))
      .catch((e) => setAuthed(e instanceof ApiError && e.status === 401 ? false : true));
  }, []);

  if (authed === null)
    return (
      <div
        className="sl-root flex min-h-svh items-center justify-center text-xs"
        style={{ color: "var(--sl-muted)" }}
      >
        loading…
      </div>
    );
  if (!authed) return <Login onAuthed={() => setAuthed(true)} />;

  const chrome: Chrome = { section, setSection, dark, setDark, onLock: lockSession };
  if (section === "compose") return <Composer chrome={chrome} />;
  if (section === "flavors") return <Flavors chrome={chrome} />;
  return <Layouts chrome={chrome} />;
}
