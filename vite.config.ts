import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import { vitePreprocess } from "@sveltejs/vite-plugin-svelte";

export default defineConfig(async () => ({
  plugins: [svelte({ preprocess: vitePreprocess() })],
  clearScreen: false,
  server: {
    port: parseInt(process.env.PORT ?? "1420"),
    strictPort: true,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
}));
