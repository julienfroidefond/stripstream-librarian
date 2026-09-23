import "@testing-library/jest-dom/vitest";

import { cleanup } from "@testing-library/react";
import { afterEach, vi } from "vitest";

afterEach(() => {
  cleanup();
});

vi.mock("next/link", async () => {
  const React = await import("react");
  return {
    __esModule: true,
    default: ({ href, children, ...rest }: any) =>
      React.createElement(
        "a",
        {
          href: typeof href === "string" ? href : (href?.pathname ?? "#"),
          ...rest,
        },
        children
      ),
    useLinkStatus: () => ({ pending: false }),
  };
});

vi.mock("next/image", async () => {
  const React = await import("react");
  return {
    __esModule: true,
    default: ({ src, alt, fill, sizes, priority, ...rest }: any) => {
      void fill;
      void sizes;
      void priority;
      return React.createElement("img", {
        src: typeof src === "string" ? src : "",
        alt,
        ...rest,
      });
    },
  };
});

vi.mock("next/navigation", () => ({
  useRouter: () => ({
    push: vi.fn(),
    replace: vi.fn(),
    refresh: vi.fn(),
    back: vi.fn(),
    prefetch: vi.fn(),
  }),
  usePathname: () => "/",
  useSearchParams: () => new URLSearchParams(),
  redirect: vi.fn(),
  notFound: vi.fn(),
}));

vi.mock("@/lib/i18n/context", async () => {
  const React = await import("react");
  return {
    useTranslation: () => ({
      t: (key: string, vars?: Record<string, unknown>) =>
        vars ? `${key}:${JSON.stringify(vars)}` : key,
      locale: "en",
      setLocale: vi.fn(),
    }),
    LocaleProvider: ({ children }: any) =>
      React.createElement(React.Fragment, null, children),
  };
});
