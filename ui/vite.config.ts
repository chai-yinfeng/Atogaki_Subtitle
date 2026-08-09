import { defineConfig } from "vite";

export default defineConfig({
  // Relative asset URLs work both from Tauri's embedded protocol and a static preview.
  base: "./",
  clearScreen: false,
  server: {
    host: "127.0.0.1",
    port: 1420,
    strictPort: true,
  },
  build: {
    // Tauri uses WebKit on macOS; keep syntax compatible with older supported systems.
    target: "safari13",
    rollupOptions: {
      input: {
        main: "index.html",
        overlay: "overlay.html",
      },
    },
  },
});
