/// <reference types="vitest/config" />
import { fileURLToPath } from "node:url";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

// Tauri expects a fixed port and fails if it is taken.
export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: { "@": fileURLToPath(new URL("./src", import.meta.url)) },
  },
  clearScreen: false,
  server: {
    port: 5180,
    strictPort: true,
    watch: { ignored: ["**/src-tauri/**"] },
  },
  build: { target: "es2022" },
  test: {
    environment: "jsdom",
    include: ["src/**/*.test.{ts,tsx}"],
  },
});
