import { svelte } from "@sveltejs/vite-plugin-svelte";
import { defineConfig } from "vite";

export default defineConfig({
  plugins: [svelte()],
  server: {
    proxy: {
      // In development the Rust server runs separately; `ws: true` is what
      // lets the game socket through as well as the HTTP routes.
      "/api": {
        target: "http://127.0.0.1:3030",
        changeOrigin: true,
        ws: true,
      },
    },
  },
});
