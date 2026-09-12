import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  server: {
    // Bound explicitly to IPv4. Vite's default `localhost` resolves to ::1 on
    // this machine, so anything checking 127.0.0.1 — Playwright's readiness
    // probe, curl, a proxy — never gets an answer.
    host: "127.0.0.1",
    port: 5173,
    strictPort: true,
    proxy: {
      // The API binds to loopback and has no authentication of its own yet, so
      // the dev server proxies rather than exposing it.
      "/api": { target: "http://127.0.0.1:8080", changeOrigin: true },
      "/healthz": { target: "http://127.0.0.1:8080", changeOrigin: true },
      "/readyz": { target: "http://127.0.0.1:8080", changeOrigin: true },
    },
  },
  test: {
    globals: true,
    environment: "jsdom",
    setupFiles: [],
    // Playwright owns e2e/. Vitest's default include would match `*.spec.ts`
    // there and try to run browser tests in jsdom.
    exclude: ["node_modules/**", "dist/**", "e2e/**"],
  },
});
