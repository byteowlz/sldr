import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import path from "node:path";

// The studio talks to sldr-server's API under /api. In dev we proxy to the
// running server (default :4100); in production sldr-server serves this build
// and the /api routes from the same origin.
export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: { alias: { "@": path.resolve(__dirname) } },
  server: {
    proxy: {
      "/api": {
        target: process.env.SLDR_API_URL || "http://127.0.0.1:4100",
        changeOrigin: true,
      },
    },
  },
});
