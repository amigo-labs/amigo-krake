import { defineConfig } from "vitest/config";
import { svelte } from "@sveltejs/vite-plugin-svelte";

export default defineConfig({
  // Compiles components for server-side rendering, so tests can render a
  // component to HTML with `svelte/server` without a DOM library.
  plugins: [svelte()],
  test: {
    include: ["test/**/*.test.ts"],
    environment: "node",
  },
});
