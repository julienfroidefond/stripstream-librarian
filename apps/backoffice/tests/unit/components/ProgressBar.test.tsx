import { render } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import {
  CircularProgress,
  MiniProgressBar,
  ProgressBar,
  SmartProgressBar,
} from "@/app/components/ui/ProgressBar";

function innerBar(container: HTMLElement, extraClass: string): HTMLElement | null {
  return container.querySelector(extraClass);
}

describe("ProgressBar", () => {
  it("sets the width from the value and max", () => {
    const { container } = render(<ProgressBar value={25} max={200} />);
    expect(innerBar(container, ".absolute")).toHaveStyle({ width: "12.5%" });
  });

  it("clamps the percentage between 0 and 100", () => {
    const over = render(<ProgressBar value={250} />);
    expect(innerBar(over.container, ".absolute")).toHaveStyle({ width: "100%" });

    const under = render(<ProgressBar value={-50} />);
    expect(innerBar(under.container, ".absolute")).toHaveStyle({ width: "0%" });
  });

  it("renders the rounded label when showLabel is set", () => {
    const { getByText } = render(<ProgressBar value={33.3} showLabel />);
    expect(getByText("33%")).toBeInTheDocument();
  });

  it("applies the requested variant", () => {
    const { container } = render(<ProgressBar value={10} variant="success" />);
    expect(innerBar(container, ".absolute")).toHaveClass("bg-success");
  });
});

describe("MiniProgressBar", () => {
  it("renders a filled bar with the computed width", () => {
    const { container } = render(<MiniProgressBar value={75} />);
    expect(innerBar(container, ".h-full")).toHaveStyle({ width: "75%" });
  });
});

describe("SmartProgressBar", () => {
  it("picks a variant from the percentage", () => {
    const full = render(<SmartProgressBar value={100} />);
    expect(innerBar(full.container, ".absolute")).toHaveClass("bg-success");

    const low = render(<SmartProgressBar value={10} />);
    expect(innerBar(low.container, ".absolute")).toHaveClass("bg-destructive");

    const mid = render(<SmartProgressBar value={30} />);
    expect(innerBar(mid.container, ".absolute")).toHaveClass("bg-warning");

    const normal = render(<SmartProgressBar value={80} />);
    expect(innerBar(normal.container, ".absolute")).toHaveClass("bg-primary");
  });
});

describe("CircularProgress", () => {
  it("renders the percentage label and a dash offset", () => {
    const { getByText, container } = render(<CircularProgress value={50} size={40} strokeWidth={4} />);
    expect(getByText("50%")).toBeInTheDocument();

    const circles = container.querySelectorAll("circle");
    expect(circles).toHaveLength(2);
    expect(circles[1]).toHaveAttribute("stroke-dashoffset");
  });

  it("uses the success color at 100%", () => {
    const { container } = render(<CircularProgress value={100} />);
    expect(container.querySelectorAll("circle")[1]).toHaveAttribute(
      "stroke",
      "hsl(var(--color-success))"
    );
  });
});
