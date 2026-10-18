import { render } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { RouteTransition } from "@/app/components/RouteTransition";

const navigation = vi.hoisted(() => ({ pathname: "/" }));

vi.mock("next/navigation", () => ({
  usePathname: () => navigation.pathname,
}));

describe("RouteTransition", () => {
  it("wraps its children in the entrance animation", () => {
    const { container } = render(
      <RouteTransition>
        <span>content</span>
      </RouteTransition>
    );

    const wrapper = container.firstElementChild as HTMLElement;
    expect(wrapper).toHaveClass("animate-route-in");
    expect(wrapper).toHaveTextContent("content");
  });

  it("replays the animation only when the pathname changes", () => {
    const { container, rerender } = render(
      <RouteTransition>
        <span>content</span>
      </RouteTransition>
    );
    const first = container.firstElementChild as HTMLElement;

    // Same pathname (query-string-only navigation) → no remount.
    rerender(
      <RouteTransition>
        <span>content</span>
      </RouteTransition>
    );
    expect(container.firstElementChild).toBe(first);

    // New route → the keyed wrapper is remounted, so the animation replays.
    navigation.pathname = "/books";
    rerender(
      <RouteTransition>
        <span>content</span>
      </RouteTransition>
    );
    const second = container.firstElementChild as HTMLElement;
    expect(second).not.toBe(first);
    expect(second).toHaveClass("animate-route-in");
  });
});
