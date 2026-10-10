import path from "node:path";

import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: { "@": path.resolve(import.meta.dirname, ".") },
  },
  test: {
    environment: "jsdom",
    setupFiles: ["./tests/setup.ts"],
    include: ["tests/unit/**/*.test.{ts,tsx}"],
    clearMocks: true,
    restoreMocks: true,
    coverage: {
      provider: "v8",
      reporter: ["text", "html", "lcov"],
      reportsDirectory: "./coverage",
      skipFull: false,
      include: ["lib/**/*.{ts,tsx}", "app/**/*.{ts,tsx}", "hooks/**/*.{ts,tsx}"],
      exclude: ["**/*.d.ts", "app/**/layout.tsx", "app/**/globals.css"],
    },
  },
});
