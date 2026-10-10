import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { ReportStatGrid } from "@/app/components/ui";

describe("ReportStatGrid", () => {
  it("renders one StatBox per stat", () => {
    const { container } = render(
      <ReportStatGrid
        stats={[
          { value: 3, label: "Auto matched", variant: "success" },
          { value: 1, label: "Errors", variant: "error" },
        ]}
      />
    );

    expect(screen.getByText("Auto matched")).toBeInTheDocument();
    expect(screen.getByText("3")).toBeInTheDocument();
    expect(screen.getByText("Errors")).toBeInTheDocument();
    expect(container.firstElementChild?.className).toContain("grid-cols-2");
  });

  it("accepts a custom column layout", () => {
    const { container } = render(<ReportStatGrid className="grid-cols-2 sm:grid-cols-5" stats={[]} />);
    expect(container.firstElementChild?.className).toContain("sm:grid-cols-5");
  });
});
