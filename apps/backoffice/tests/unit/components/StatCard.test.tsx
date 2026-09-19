import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { StatCard } from "@/app/(app)/_components/StatCard";

describe("StatCard", () => {
  it("renders the value and label", () => {
    render(<StatCard icon="book" label="Total books" value="1,234" color="primary" />);

    expect(screen.getByText("1,234")).toBeInTheDocument();
    expect(screen.getByText("Total books")).toBeInTheDocument();
  });

  it("renders an icon", () => {
    const { container } = render(
      <StatCard icon="series" label="Series" value="12" color="primary" />
    );

    expect(container.querySelector("svg")).toBeInTheDocument();
  });

  it("maps the color to the matching classes", () => {
    const { container } = render(
      <StatCard icon="book" label="Books" value="1" color="success" />
    );

    expect(container.querySelector(".w-10")).toHaveClass("bg-success/10", "text-success");
  });

  it("uses the warning palette", () => {
    const { container } = render(
      <StatCard icon="error" label="Errors" value="3" color="warning" />
    );

    expect(container.querySelector(".w-10")).toHaveClass("bg-warning/10", "text-warning");
  });
});
