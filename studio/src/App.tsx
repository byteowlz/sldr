import { useEffect, useState } from "react";
import { Moon, Sun, Lock, Presentation } from "lucide-react";
import { api, ApiError, setToken, clearToken } from "@/lib/api";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Tabs, TabsList, TabsTrigger, TabsContent } from "@/components/ui/tabs";
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
          <CardDescription>
            Enter the server token (SLDR_API_TOKEN).
          </CardDescription>
        </CardHeader>
        <CardContent>
          <form onSubmit={submit} className="space-y-3">
            <div className="space-y-1.5">
              <Label htmlFor="token">Token</Label>
              <Input
                id="token"
                type="password"
                value={token}
                onChange={(e) => setTok(e.target.value)}
                autoFocus
              />
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
      <div className="flex min-h-svh items-center justify-center text-muted-foreground">
        Loading…
      </div>
    );
  if (!authed) return <Login onAuthed={() => setAuthed(true)} />;

  return (
    <div className="flex min-h-svh flex-col">
      <Tabs defaultValue="decks" className="flex flex-1 flex-col gap-0">
        <header className="sticky top-0 z-10 flex items-center gap-3 border-b bg-background/80 px-4 py-2.5 backdrop-blur">
          <span className="flex items-center gap-2 font-semibold">
            <Presentation className="size-5 text-primary" /> sldr&nbsp;studio
          </span>
          <TabsList className="ml-2">
            <TabsTrigger value="decks">Decks</TabsTrigger>
            <TabsTrigger value="flavors">Flavors</TabsTrigger>
            <TabsTrigger value="layouts">Layouts</TabsTrigger>
          </TabsList>
          <div className="ml-auto flex items-center gap-1">
            <Button
              variant="ghost"
              size="icon"
              onClick={() => setDark(!dark)}
              aria-label="Toggle theme"
            >
              {dark ? <Sun className="size-4" /> : <Moon className="size-4" />}
            </Button>
            <Button
              variant="ghost"
              size="icon"
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
        <main className="flex-1 overflow-auto p-4 sm:p-6">
          <div className="mx-auto max-w-6xl">
            <TabsContent value="decks">
              <Decks />
            </TabsContent>
            <TabsContent value="flavors">
              <Flavors />
            </TabsContent>
            <TabsContent value="layouts">
              <Layouts />
            </TabsContent>
          </div>
        </main>
      </Tabs>
    </div>
  );
}
