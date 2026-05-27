import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import { vitePreprocess } from "@sveltejs/vite-plugin-svelte";
import path from "path";

/**
 * Standalone Vite config for browser-based component testing.
 *
 * Redirects all @tauri-apps/api/* imports to src/mocks/* so the Svelte
 * components render in a regular browser without any Tauri runtime.
 *
 * Usage:
 *   npm run standalone:dev    → http://localhost:1421/hud.html
 *   npm run standalone:build  → dist-standalone/
 */
export default defineConfig({
  plugins: [svelte({ preprocess: vitePreprocess() })],

  resolve: {
    alias: {
      "@tauri-apps/api/core": path.resolve(__dirname, "src/mocks/tauri-core.ts"),
      "@tauri-apps/api/window": path.resolve(__dirname, "src/mocks/tauri-window.ts"),
      "@tauri-apps/api/app": path.resolve(__dirname, "src/mocks/tauri-app.ts"),
      "@tauri-apps/api/event": path.resolve(__dirname, "src/mocks/tauri-event.ts"),
    },
  },

  server: {
    port: 1422,
    strictPort: false,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },

  build: {
    outDir: "dist-standalone",
    rollupOptions: {
      input: {
        hud: path.resolve(__dirname, "hud.html"),
        settings: path.resolve(__dirname, "settings.html"),
      },
    },
  },
});
