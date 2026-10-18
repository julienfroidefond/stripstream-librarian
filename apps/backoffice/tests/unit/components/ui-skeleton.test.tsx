import { render } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { Skeleton } from "@/app/components/ui";

describe("Skeleton", () => {
  it("renders a shimmering placeholder and merges the shape classes", () => {
    const { container } = render(<Skeleton className="h-20 rounded-xl" />);
    const el = container.firstElementChild as HTMLElement;
    expect(el).toHaveClass("skeleton-shimmer", "bg-muted", "h-20", "rounded-xl");
  });

  it("is hidden from assistive tech and defaults to the base radius", () => {
    const { container } = render(<Skeleton />);
    const el = container.firstElementChild as HTMLElement;
    expect(el).toHaveAttribute("aria-hidden", "true");
    expect(el).toHaveClass("rounded-md", "skeleton-shimmer", "bg-muted");
  });
});
