import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { CountUp } from "@/app/components/motion/CountUp";
import { GlowLayer } from "@/app/components/motion/GlowLayer";
import { JacketGrid } from "@/app/components/motion/JacketGrid";
import { Magnetic } from "@/app/components/motion/Magnetic";
import { ScrollProgress } from "@/app/components/motion/ScrollProgress";

vi.mock("framer-motion", async (importOriginal) => {
  const actual = await importOriginal<typeof import("framer-motion")>();
  return { ...actual, animate: vi.fn(actual.animate) };
});

// Imported after the mock so it is the mocked function.
const { animate } = await import("framer-motion");

/** Minimal IntersectionObserver double that hands back its callback. */
function stubIntersectionObserver() {
  const state: {
    callback: IntersectionObserverCallback | null;
    observed: Element[];
    disconnected: number;
  } = { callback: null, observed: [], disconnected: 0 };

  class FakeIntersectionObserver {
    constructor(callback: IntersectionObserverCallback) {
      state.callback = callback;
    }
    observe = (el: Element) => {
      state.observed.push(el);
    };
    unobserve = vi.fn();
    disconnect = () => {
      state.disconnected += 1;
    };
    takeRecords = () => [];
    root = null;
    rootMargin = "";
    thresholds = [];
  }

  vi.stubGlobal("IntersectionObserver", FakeIntersectionObserver);
  return state;
}

function intersect(state: ReturnType<typeof stubIntersectionObserver>, el: Element) {
  state.callback?.(
    [{ target: el, isIntersecting: true } as unknown as IntersectionObserverEntry],
    {} as IntersectionObserver,
  );
}

afterEach(() => {
  vi.unstubAllGlobals();
  vi.mocked(animate).mockClear();
});

describe("CountUp", () => {
  it("renders the final figure without an observer (no-JS safe)", () => {
    vi.stubGlobal("IntersectionObserver", undefined);

    render(<CountUp value={1234} locale="en" />);

    expect(screen.getByText("1,234")).toBeInTheDocument();
  });

  it("counts from zero to the value once it enters the viewport", () => {
    const state = stubIntersectionObserver();
    const { container } = render(<CountUp value={120} locale="en" duration={1} />);
    const el = container.firstElementChild!;

    expect(state.observed).toContain(el);

    intersect(state, el);

    expect(animate).toHaveBeenCalledWith(
      0,
      120,
      expect.objectContaining({
        duration: 1,
        ease: [0.22, 1, 0.36, 1],
      }),
    );
  });

  it("skips the animation for a zero value", () => {
    const state = stubIntersectionObserver();
    const { container } = render(<CountUp value={0} locale="en" />);

    intersect(state, container.firstElementChild!);

    expect(animate).not.toHaveBeenCalled();
  });

  it("formats byte values", () => {
    vi.stubGlobal("IntersectionObserver", undefined);

    render(<CountUp value={2048} format="bytes" locale="en" />);

    expect(screen.getByText("2.0 KB")).toBeInTheDocument();
  });
});

describe("Magnetic", () => {
  it("renders its children", () => {
    render(<Magnetic><span>Lancer</span></Magnetic>);

    expect(screen.getByText("Lancer")).toBeInTheDocument();
  });

  it("leans toward the pointer, then springs back on leave", async () => {
    const { container } = render(
      <Magnetic strength={0.5} max={20}>
        <span>Lancer</span>
      </Magnetic>,
    );
    const wrapper = container.firstElementChild as HTMLElement;

    vi.spyOn(wrapper, "getBoundingClientRect").mockReturnValue({
      top: 0,
      left: 0,
      right: 200,
      bottom: 100,
      width: 200,
      height: 100,
      x: 0,
      y: 0,
      toJSON: () => ({}),
    } as DOMRect);

    fireEvent.pointerMove(wrapper, { clientX: 200, clientY: 100 });

    await waitFor(() => {
      expect(wrapper.style.transform).toContain("translateX");
    });

    fireEvent.pointerLeave(wrapper);

    // Framer Motion collapses a zero offset back to `transform: none`.
    await waitFor(() => {
      expect(wrapper.style.transform).toMatch(/none|translateX\(0px\)/);
    });
  });
});

describe("GlowLayer", () => {
  it("tracks the pointer within its host", async () => {
    const { container } = render(
      <div className="group relative">
        <GlowLayer />
      </div>,
    );
    const host = container.firstElementChild as HTMLElement;
    const layer = host.firstElementChild as HTMLElement;

    expect(layer).toHaveAttribute("aria-hidden");

    vi.spyOn(host, "getBoundingClientRect").mockReturnValue({
      top: 10,
      left: 20,
      right: 220,
      bottom: 110,
      width: 200,
      height: 100,
      x: 20,
      y: 10,
      toJSON: () => ({}),
    } as DOMRect);

    fireEvent.pointerMove(host, { clientX: 70, clientY: 60 });

    await waitFor(() => {
      expect(layer.style.getPropertyValue("--glow-x")).toBe("50px");
      expect(layer.style.getPropertyValue("--glow-y")).toBe("50px");
    });
  });
});

describe("ScrollProgress", () => {
  it("renders a hairline anchored to the left edge", () => {
    const { container } = render(<ScrollProgress className="h-[2px]" />);
    const bar = container.firstElementChild as HTMLElement;

    expect(bar).toHaveClass("h-[2px]");
    expect(bar.style.transformOrigin).toBe("0% 50%");
  });
});

describe("JacketGrid", () => {
  const card = (id: string) => <div key={id}>{id}</div>;

  it("marks the grid and shows every card without an observer (no-JS safe)", () => {
    vi.stubGlobal("IntersectionObserver", undefined);

    const { container } = render(
      <JacketGrid className="grid grid-cols-3">
        {[card("a"), card("b"), card("c")]}
      </JacketGrid>,
    );

    const grid = container.firstElementChild as HTMLElement;
    const cards = Array.from(grid.children) as HTMLElement[];

    expect(grid).toHaveAttribute("data-jackets");
    expect(grid).toHaveClass("grid-cols-3");

    // jsdom has no layout: every card shares the same row, so the delay is
    // simply the card index — the row unfolds from left to right.
    expect(cards.map((el) => el.dataset.jacket)).toEqual(["revealed", "revealed", "revealed"]);
    expect(cards.map((el) => el.style.getPropertyValue("--jacket-delay"))).toEqual([
      "0ms",
      "90ms",
      "180ms",
    ]);
  });

  it("watches each card and only reveals it once it enters the viewport", () => {
    const state = stubIntersectionObserver();

    const { container } = render(
      <JacketGrid stagger={120}>
        {[card("a"), card("b"), card("c")]}
      </JacketGrid>,
    );
    const cards = Array.from(container.firstElementChild!.children) as HTMLElement[];

    expect(state.observed).toHaveLength(3);
    expect(cards.every((el) => el.dataset.jacket === undefined)).toBe(true);

    // A later card lands when it scrolls into view, with its row-rank delay.
    intersect(state, cards[2]);

    expect(cards[2].dataset.jacket).toBe("revealed");
    expect(cards[2].style.getPropertyValue("--jacket-delay")).toBe("240ms");
    expect(cards[0].dataset.jacket).toBeUndefined();
  });

  it("picks up cards mounted after the first render", async () => {
    const state = stubIntersectionObserver();

    const { container, rerender } = render(<JacketGrid>{[card("a")]}</JacketGrid>);

    rerender(<JacketGrid>{[card("a"), card("b")]}</JacketGrid>);

    await waitFor(() => {
      expect(state.observed).toHaveLength(2);
    });
    expect(container.firstElementChild!.children[1]).toBe(state.observed[1]);
  });
});
