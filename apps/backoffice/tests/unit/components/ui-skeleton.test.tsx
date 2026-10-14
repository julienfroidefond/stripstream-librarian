import { render } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { Skeleton } from "@/app/components/ui";

describe("Skeleton", () => {
  it("renders a pulsing placeholder and merges the shape classes", () => {
    const { container } = render(<Skeleton className="h-20 rounded-xl" />);
    const el = container.firstElementChild as HTMLElement;
    expect(el).toHaveClass("animate-pulse", "bg-muted", "h-20", "rounded-xl");
  });

  it("is hidden from assistive tech and defaults to the base radius", () => {
    const { container } = render(<Skeleton />);
    const el = container.firstElementChild as HTMLElement;
    expect(el).toHaveAttribute("aria-hidden", "true");
    expect(el).toHaveClass("rounded-md", "animate-pulse", "bg-muted");
  });
});
