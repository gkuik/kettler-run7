import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

// Hôte optionnel utilisé par Tauri pour le dev mobile ; inoffensif sur desktop.
const host = process.env.TAURI_DEV_HOST;

// Configuration Vite adaptée à Tauri : port fixe 1430 (attendu par `tauri dev` ; 1420 est pris par OpenGitClient),
// on ignore le dossier `src-tauri` pour éviter des rebuilds inutiles.
export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  // macOS 12 : la WebView est le WebKit du système, celui de Safari 15 au pire.
  build: {
    target: ["es2020", "safari15"],
    cssTarget: ["safari15"],
  },
  server: {
    port: 1430,
    strictPort: true,
    host: host || false,
    hmr: host
      ? { protocol: "ws", host, port: 1431 }
      : undefined,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
});
