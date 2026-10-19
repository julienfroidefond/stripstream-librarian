import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { ViewModeToggle } from "@/app/components/ViewModeToggle";
import { VIEW_MODE_COOKIE } from "@/lib/viewMode";

describe("ViewModeToggle", () => {
  it("offers to switch to the list when currently in grid mode", () => {
    render(<ViewModeToggle viewMode="grid" />);

    const button = screen.getByRole("button", { name: "books.display.list" });
    expect(button).toHaveAttribute("aria-pressed", "false");
  });

  it("offers to switch to the grid when currently in list mode", () => {
    render(<ViewModeToggle viewMode="list" />);

    const button = screen.getByRole("button", { name: "books.display.grid" });
    expect(button).toHaveAttribute("aria-pressed", "true");
  });

  it("persists the next mode in the view_mode cookie on click", () => {
    document.cookie = `${VIEW_MODE_COOKIE}=;path=/;max-age=0`;
    render(<ViewModeToggle viewMode="grid" />);

    fireEvent.click(screen.getByRole("button", { name: "books.display.list" }));

    expect(document.cookie).toContain(`${VIEW_MODE_COOKIE}=list`);
  });

  it("writes the grid value when switching back from the list", () => {
    document.cookie = `${VIEW_MODE_COOKIE}=list;path=/`;
    render(<ViewModeToggle viewMode="list" />);

    fireEvent.click(screen.getByRole("button", { name: "books.display.grid" }));

    expect(document.cookie).toContain(`${VIEW_MODE_COOKIE}=grid`);
  });
});
