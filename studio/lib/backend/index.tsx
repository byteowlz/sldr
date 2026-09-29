// Backend selection + React plumbing. Standalone by default; inside an Oqto
// host frame (`?host=oqto`, or a parent frame that answers the handshake)
// the SDK connection wins. Sections only ever call `useBackend()`.

import { createContext, useContext, useEffect, useState, type ReactNode } from "react";
import { connectOqtoApp } from "@byteowlz/oqto-app-sdk";
import type { Backend } from "./types";
import { httpBackend } from "./http";
import { oqtoBackend } from "./oqto";

export type { Backend, Preview, BackendId } from "./types";
export { BackendError } from "./types";

const Ctx = createContext<Backend | null>(null);

function wantsOqto(): boolean {
  if (window.parent === window) return false;
  const p = new URLSearchParams(location.search);
  return p.get("host") === "oqto" || !!document.referrer;
}

/** Resolve the backend once. Never throws: a failed Oqto handshake falls
 * back to standalone with a console note, since that is what a stray
 * iframe embed of the standalone studio should get. */
export async function detectBackend(): Promise<Backend> {
  if (wantsOqto()) {
    try {
      const host = await connectOqtoApp({ handshakeTimeoutMs: 4000 });
      return oqtoBackend(host);
    } catch (e) {
      console.info("sldr studio: not an Oqto mount, using standalone backend", e);
    }
  }
  return httpBackend();
}

export function BackendProvider({ children, fallback }: { children: ReactNode; fallback?: ReactNode }) {
  const [backend, setBackend] = useState<Backend | null>(null);
  useEffect(() => {
    let live = true;
    detectBackend().then((b) => live && setBackend(b));
    return () => {
      live = false;
    };
  }, []);
  if (!backend) return <>{fallback ?? null}</>;
  return <Ctx.Provider value={backend}>{children}</Ctx.Provider>;
}

export function useBackend(): Backend {
  const b = useContext(Ctx);
  if (!b) throw new Error("useBackend outside <BackendProvider>");
  return b;
}
