import { act, render } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { MotionReveal } from "@/app/components/MotionReveal";

function addRevealable(): HTMLElement {
  const el = document.createElement("div");
  el.setAttribute("data-reveal", "");
  document.body.appendChild(el);
  return el;
}

afterEach(() => {
  document.querySelectorAll("[data-reveal]").forEach((el) => el.remove());
  document.documentElement.classList.remove("js-motion");
  vi.unstubAllGlobals();
});

describe("MotionReveal", () => {
  it("flags the document and reveals everything when IntersectionObserver is unavailable", () => {
    vi.stubGlobal("IntersectionObserver", undefined);
    const el = addRevealable();

    render(<MotionReveal />);

    expect(document.documentElement).toHaveClass("js-motion");
    expect(el).toHaveClass("is-revealed");
  });

  it("reveals an element only once it intersects the viewport", () => {
    let callback: IntersectionObserverCallback = () => {};
    const observe = vi.fn();
    const unobserve = vi.fn();

    class FakeIntersectionObserver {
      constructor(cb: IntersectionObserverCallback) {
        callback = cb;
      }
      observe = observe;
      unobserve = unobserve;
      disconnect = vi.fn();
    }
    vi.stubGlobal("IntersectionObserver", FakeIntersectionObserver);

    const el = addRevealable();
    render(<MotionReveal />);

    expect(observe).toHaveBeenCalledWith(el);
    expect(el).not.toHaveClass("is-revealed");

    act(() => {
      callback(
        [{ target: el, isIntersecting: true } as unknown as IntersectionObserverEntry],
        {} as IntersectionObserver
      );
    });

    expect(el).toHaveClass("is-revealed");
    expect(unobserve).toHaveBeenCalledWith(el);
  });
});
