import { defineConfig } from "vite";

const host = process.env.TAURI_DEV_HOST;

// Settings follow the Tauri 2 + Vite guide: fixed port, no screen clearing.
export default defineConfig({
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host ? { protocol: "ws", host, port: 1421 } : undefined,
    watch: { ignored: ["**/src-tauri/**"] },
  },
  envPrefix: ["VITE_", "TAURI_ENV_*"],
  build: {
    target: "chrome105",
    minify: !process.env.TAURI_ENV_DEBUG,
    sourcemap: !!process.env.TAURI_ENV_DEBUG,
  },
});
