import { useEffect, useState } from "react";
import { Moon, Sun, Lock, Presentation } from "lucide-react";
import { useQuery } from "@tanstack/react-query";
import { api, ApiError, setToken, clearToken } from "@/lib/api";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Decks } from "./sections/Decks";
import { Layouts } from "./sections/Layouts";
import { Flavors } from "./sections/Flavors";

type SectionId = "decks" | "flavors" | "layouts";
const SECTIONS: { id: SectionId; label: string }[] = [
  { id: "decks", label: "Decks" },
  { id: "flavors", label: "Flavors" },
  { id: "layouts", label: "Layouts" },
];

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
    <div className="flex min-h-svh items-center justify-center p-6">
      <Card className="w-full max-w-sm">
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Presentation className="size-5" /> sldr studio
          </CardTitle>
          <CardDescription>Enter the server token (SLDR_API_TOKEN).</CardDescription>
        </CardHeader>
        <CardContent>
          <form onSubmit={submit} className="space-y-3">
            <div className="space-y-1.5">
              <Label htmlFor="token">Token</Label>
              <Input id="token" type="password" value={token} onChange={(e) => setTok(e.target.value)} autoFocus />
            </div>
            {err && <p className="text-sm text-destructive">{err}</p>}
            <Button type="submit" disabled={busy} className="w-full">
              {busy ? "Unlocking…" : "Unlock"}
            </Button>
          </form>
        </CardContent>
      </Card>
    </div>
  );
}

export default function App() {
  const [dark, setDark] = useDark();
  const [authed, setAuthed] = useState<boolean | null>(null);
  const [section, setSection] = useState<SectionId>("decks");
  const [flavor, setFlavor] = useState("default");
  const flavors = useQuery({ queryKey: ["flavors"], queryFn: api.flavors, enabled: authed === true });

  useEffect(() => {
    api
      .slides()
      .then(() => setAuthed(true))
      .catch((e) => setAuthed(e instanceof ApiError && e.status === 401 ? false : true));
  }, []);

  if (authed === null)
    return (
      <div className="flex min-h-svh items-center justify-center text-sm text-muted-foreground">
        Loading…
      </div>
    );
  if (!authed) return <Login onAuthed={() => setAuthed(true)} />;

  return (
    <div className="grid h-svh grid-rows-[40px_minmax(0,1fr)] bg-background text-foreground">
      {/* Top bar */}
      <header className="flex items-center gap-3 border-b bg-card px-3">
        <span className="flex items-center gap-1.5 text-sm font-semibold">
          <Presentation className="size-4 text-primary" />
          sldr
        </span>
        <nav className="flex items-center overflow-hidden rounded-sm border">
          {SECTIONS.map((s) => (
            <button
              key={s.id}
              onClick={() => setSection(s.id)}
              className={cn(
                "border-r px-3 py-1 text-xs last:border-r-0",
                section === s.id
                  ? "bg-primary text-primary-foreground"
                  : "text-muted-foreground hover:bg-foreground/[0.04]",
              )}
            >
              {s.label}
            </button>
          ))}
        </nav>

        <div className="ml-auto flex items-center gap-2">
          <Select value={flavor} onValueChange={setFlavor}>
            <SelectTrigger className="h-7 w-44 text-xs" size="sm">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="default">default</SelectItem>
              {flavors.data?.map((f) => (
                <SelectItem key={f.name} value={f.name}>
                  {f.display_name || f.name}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          <Button variant="ghost" size="icon" className="size-7" onClick={() => setDark(!dark)} aria-label="Theme">
            {dark ? <Sun className="size-4" /> : <Moon className="size-4" />}
          </Button>
          <Button
            variant="ghost"
            size="icon"
            className="size-7"
            aria-label="Lock"
            onClick={() => {
              clearToken();
              setAuthed(false);
            }}
          >
            <Lock className="size-4" />
          </Button>
        </div>
      </header>

      {/* Body */}
      <div className="min-h-0">
        {section === "decks" && <Decks flavor={flavor} />}
        {section === "flavors" && <Flavors />}
        {section === "layouts" && <Layouts />}
      </div>
    </div>
  );
}
