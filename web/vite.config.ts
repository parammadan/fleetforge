import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  server: {
    port: 5173,
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
  },
});
