import { defineConfig } from "vite";

export default defineConfig({
  // Standalone serves at "/"; an embedding host builds with MCR_BASE=/mcr/ so the
  // bundle's assets resolve under that sub-path.
  base: process.env.MCR_BASE ?? "/",
  clearScreen: false,
  server: { port: 5173, strictPort: true },
  test: {
    environment: "jsdom",
    globals: true,
  },
});
