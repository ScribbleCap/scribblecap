import { defineConfig } from "vitest/config";
import { svelte } from '@sveltejs/vite-plugin-svelte';

// Tauri serves the dev build on a fixed port and needs the same output on every OS.
export default defineConfig({
  clearScreen: false,
  plugins: [svelte()],
  resolve: { conditions: ['browser'] },
  server: { port: 5173, strictPort: true },
  build: { target: ["es2022", "safari15"], outDir: "dist", emptyOutDir: true },
  test: { environment: 'jsdom', setupFiles: ['src/test-setup.ts'] },
});
