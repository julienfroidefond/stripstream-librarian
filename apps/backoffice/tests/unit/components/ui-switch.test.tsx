import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { Switch } from "@/app/components/ui";

describe("Switch", () => {
  it("renders a checkbox bound to `checked`", () => {
    const { container } = render(<Switch checked readOnly />);
    const input = container.querySelector<HTMLInputElement>('input[type="checkbox"]');
    expect(input).not.toBeNull();
    expect(input?.checked).toBe(true);
  });

  it("fires onChange when clicked", () => {
    const onChange = vi.fn();
    render(<Switch checked={false} onChange={onChange} />);

    fireEvent.click(screen.getByRole("checkbox"));

    expect(onChange).toHaveBeenCalledTimes(1);
  });

  it("renders the label and makes the row clickable", () => {
    const onChange = vi.fn();
    render(<Switch label={<span>Notifications</span>} labelPosition="left" onChange={onChange} />);

    expect(screen.getByText("Notifications")).toBeInTheDocument();
    fireEvent.click(screen.getByText("Notifications"));

    expect(onChange).toHaveBeenCalledTimes(1);
  });

  it("uses the compact track for the sm size", () => {
    const { container } = render(<Switch size="sm" />);
    expect(container.querySelector(".w-9")).not.toBeNull();
    expect(container.querySelector(".after\\:h-4")).not.toBeNull();
  });
});
