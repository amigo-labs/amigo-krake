import { defineConfig } from "vitest/config";
import { svelte } from "@sveltejs/vite-plugin-svelte";

export default defineConfig({
  // Compiles components for tests: server-side rendering with
  // `svelte/server` in the node environment, and mounting in files that ask
  // for `happy-dom`. The browser condition makes `svelte` resolve to its
  // client runtime there (`mount` is not available on the server one).
  plugins: [svelte()],
  resolve: { conditions: ["browser"] },
  test: {
    include: ["test/**/*.test.ts"],
    environment: "node",
  },
});
