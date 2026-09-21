import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

// ~ resolves to src/ — matches the tsconfig paths alias so both TypeScript and
// Vite resolve `~/lib/ui` to `src/lib/ui` identically.
// import.meta.dirname is available in Node 21.2+ (we require Node 22).
export default defineConfig({
  plugins: [tailwindcss(), react()],
  resolve: {
    alias: {
      "~": new URL("./src", import.meta.url).pathname,
    },
  },
  server: {
    port: 3100,
    proxy: {
      // Dev: forward the SPA's /api and /cable to the Rails API (default :3000).
      // Override the Rails origin with VITE_API_TARGET if it runs on another port.
      "/api": {
        target: process.env.VITE_API_TARGET || "http://localhost:3000",
        changeOrigin: true,
      },
      "/cable": {
        target: (process.env.VITE_API_TARGET || "http://localhost:3000").replace(
          /^http/,
          "ws",
        ),
        ws: true,
        changeOrigin: true,
      },
    },
  },
  build: {
    target: "esnext",
    sourcemap: "hidden",
    manifest: true,
  },
});
