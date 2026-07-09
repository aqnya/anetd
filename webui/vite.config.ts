import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

export default defineConfig({
  plugins: [svelte()],
  root: ".",
  base: "./",
  build: {
    outDir: "../module/webroot",
    emptyOutDir: true,
    target: "es2022",
  },
});
