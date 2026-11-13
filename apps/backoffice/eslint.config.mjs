import nextCoreWebVitals from "eslint-config-next/core-web-vitals";

const config = [
  ...nextCoreWebVitals,
  {
    rules: {
      // These rules come from the React Compiler-aware react-hooks v7 preset.
      // The codebase relies on a few long-standing idioms they flag (mounted
      // flags, latest-value refs); keep them visible as warnings until the
      // patterns are migrated, without blocking the lint gate.
      "react-hooks/set-state-in-effect": "warn",
      "react-hooks/refs": "warn",
      "react-hooks/immutability": "warn",
    },
  },
  {
    // Cache invalidation goes through `invalidateAfter` (lib/cache, ADR 0021).
    files: ["app/**/*.{ts,tsx}", "lib/**/*.{ts,tsx}"],
    ignores: ["lib/cache/**", "app/(app)/settings/components/TokensTab.tsx"],
    rules: {
      "no-restricted-imports": [
        "error",
        {
          paths: [
            {
              name: "next/cache",
              importNames: ["revalidateTag", "revalidatePath", "updateTag"],
              message: "Use invalidateAfter() from @/lib/cache.",
            },
          ],
        },
      ],
    },
  },
  {
    ignores: ["coverage/**", "playwright-report/**", "test-results/**"],
  },
];

export default config;
