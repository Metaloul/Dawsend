import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

// Interface de l'appli Tauri : servie par Vite en développement, intégrée au binaire
// (dist/) en production.
export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  server: { port: 5173, strictPort: true },
  build: { target: "es2022", outDir: "dist", emptyOutDir: true },
});
