import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

export default defineConfig({
  plugins: [
    svelte(),
    {
      name: "strip-crossorigin-for-file-protocol",
      enforce: "post",
      transformIndexHtml(html) {
        // `crossorigin` breaks resource loading when the WebView opens
        // index.html via file:// (KSU / MMRL manager).  Strip it from
        // <script>, <link>, and <style> tags so the browser fetches them
        // without a CORS check.
        return html.replace(
          /(<(?:script|link|style)\b[^>]*?)\s+crossorigin(?:\s*=\s*"[^"]*")?\s*/gi,
          "$1 "
        );
      },
    },
  ],
  root: ".",
  base: "./",
  build: {
    outDir: "../module/webroot",
    emptyOutDir: true,
    target: "es2022",
  },
});
