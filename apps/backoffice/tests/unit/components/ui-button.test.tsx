import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { Button, buttonClasses } from "@/app/components/ui";

describe("buttonClasses", () => {
  it("composes variant, size and extra classes", () => {
    const cls = buttonClasses({ variant: "warning", size: "sm", className: "mt-4" });

    expect(cls).toContain("bg-warning");
    expect(cls).toContain("text-warning-foreground");
    expect(cls).toContain("h-9");
    expect(cls).toContain("mt-4");
  });

  it("defaults to the primary, medium style", () => {
    const cls = buttonClasses();

    expect(cls).toContain("bg-primary");
    expect(cls).toContain("h-10");
  });
});

describe("Button", () => {
  it("renders a native button using the shared classes", () => {
    render(<Button variant="outline">Go</Button>);

    expect(screen.getByRole("button", { name: "Go" })).toHaveClass("ring-1", "h-10");
  });
});
