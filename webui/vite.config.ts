import { defineConfig } from "vite";

export default defineConfig({
  root: ".",
  base: "./",
  build: {
    outDir: "../module/webroot",
    emptyOutDir: true,
    target: "es2022",
    minify: "esbuild",
  },
});
