import { defineConfig } from "vitest/config";

// Tauri serves the dev build on a fixed port and needs the same output on every OS.
export default defineConfig({
  clearScreen: false,
  server: { port: 5173, strictPort: true },
  build: { target: ["es2022", "safari15"], outDir: "dist", emptyOutDir: true },
  test: { passWithNoTests: true },
});
