import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

// Tauri sets TAURI_DEV_HOST when developing against a phone.
const host = process.env.TAURI_DEV_HOST;
// Where `npm run dev` forwards /ws, so the browser can use same-origin.
const server = process.env.PORCH_DEV_SERVER ?? "http://127.0.0.1:7878";

export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  build: { target: "es2022", outDir: "dist" },
  server: {
    port: 1420,
    strictPort: true,
    host: host || "127.0.0.1",
    hmr: host ? { protocol: "ws", host, port: 1421 } : undefined,
    watch: { ignored: ["**/src-tauri/**"] },
    proxy: { "/ws": { target: server, ws: true } },
  },
});
