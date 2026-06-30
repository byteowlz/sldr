import { useEffect, useState } from "react";
import { Moon, Sun, Lock } from "lucide-react";
import { api, ApiError, setToken, clearToken } from "@/lib/api";
import { Button, Input, Spinner } from "@/components/ui";
import { Decks } from "@/sections/Decks";
import { Layouts } from "@/sections/Layouts";
import { Flavors } from "@/sections/Flavors";

const SECTIONS = [
  { id: "decks", label: "Decks", el: <Decks /> },
  { id: "flavors", label: "Flavors", el: <Flavors /> },
  { id: "layouts", label: "Layouts", el: <Layouts /> },
] as const;

function useDark() {
  const [dark, setDark] = useState(
    () => localStorage.getItem("sldr:theme") !== "light",
  );
  useEffect(() => {
    document.documentElement.classList.toggle("dark", dark);
    localStorage.setItem("sldr:theme", dark ? "dark" : "light");
  }, [dark]);
  return [dark, setDark] as const;
}

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
      setErr(e instanceof ApiError && e.status === 401 ? "Invalid token" : String(e));
      setBusy(false);
    }
  };
  return (
    <div className="flex h-full items-center justify-center p-6">
      <form onSubmit={submit} className="w-full max-w-sm space-y-3">
        <h1 className="text-xl font-semibold">sldr studio</h1>
        <p className="text-sm text-[var(--color-muted)]">
          Enter the server token (SLDR_API_TOKEN).
        </p>
        <Input
          type="password"
          placeholder="token"
          value={token}
          onChange={(e) => setTok(e.target.value)}
          autoFocus
        />
        {err && <div className="text-sm text-[var(--color-danger)]">{err}</div>}
        <Button type="submit" disabled={busy} className="w-full">
          {busy ? <Spinner /> : "Unlock"}
        </Button>
      </form>
    </div>
  );
}

export default function App() {
  const [dark, setDark] = useDark();
  const [authed, setAuthed] = useState<boolean | null>(null);
  const [active, setActive] = useState<(typeof SECTIONS)[number]["id"]>("decks");

  useEffect(() => {
    api
      .slides()
      .then(() => setAuthed(true))
      .catch((e) =>
        setAuthed(e instanceof ApiError && e.status === 401 ? false : true),
      );
  }, []);

  if (authed === null)
    return (
      <div className="flex h-full items-center justify-center">
        <Spinner />
      </div>
    );
  if (!authed) return <Login onAuthed={() => setAuthed(true)} />;

  return (
    <div className="flex h-full flex-col">
      <header className="flex items-center gap-2 border-b border-[var(--color-border)] px-4 py-3">
        <span className="font-semibold">sldr studio</span>
        <nav className="ml-2 flex gap-1">
          {SECTIONS.map((s) => (
            <button
              key={s.id}
              onClick={() => setActive(s.id)}
              className={
                "rounded-md px-3 py-1.5 text-sm transition-colors " +
                (active === s.id
                  ? "bg-[var(--color-card)] font-medium"
                  : "text-[var(--color-muted)] hover:bg-[var(--color-card)]")
              }
            >
              {s.label}
            </button>
          ))}
        </nav>
        <div className="ml-auto flex items-center gap-1">
          <Button variant="ghost" onClick={() => setDark(!dark)}>
            {dark ? <Sun size={16} /> : <Moon size={16} />}
          </Button>
          <Button
            variant="ghost"
            onClick={() => {
              clearToken();
              setAuthed(false);
            }}
            title="Lock"
          >
            <Lock size={16} />
          </Button>
        </div>
      </header>
      <main className="flex-1 overflow-auto p-4 sm:p-6">
        {SECTIONS.find((s) => s.id === active)?.el}
      </main>
    </div>
  );
}
