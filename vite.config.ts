import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import conf from "./src-tauri/tauri.conf.json";

export default defineConfig({
  plugins: [react()],
  define: { __APP_VERSION__: JSON.stringify(conf.version) },
  clearScreen: false,
  server: { port: 5174, strictPort: true },
  build: { target: "es2021", minify: "esbuild", sourcemap: false },
});
