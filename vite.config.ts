import { defineConfig } from "vite";
import { fileURLToPath } from "node:url";

export default defineConfig({
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  build: {
    target: "chrome105",
    rollupOptions: {
      input: {
        main: fileURLToPath(new URL("./index.html", import.meta.url)),
        bubble: fileURLToPath(new URL("./bubble.html", import.meta.url)),
        snip: fileURLToPath(new URL("./snip.html", import.meta.url)),
      },
    },
  },
});
