import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import "./styles/globals.css";
import "./styles/composer.css";
import "./styles/board.css";
import App from "./App";
import { BackendProvider } from "@/lib/backend";

const queryClient = new QueryClient({
  defaultOptions: { queries: { retry: false, refetchOnWindowFocus: false } },
});

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <QueryClientProvider client={queryClient}>
      <BackendProvider fallback={<div className="sl-root flex min-h-svh items-center justify-center text-xs">connecting…</div>}>
        <App />
      </BackendProvider>
    </QueryClientProvider>
  </StrictMode>,
);
