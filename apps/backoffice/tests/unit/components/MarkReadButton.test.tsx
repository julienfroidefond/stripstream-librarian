import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { MarkReadButton } from "@/app/components/MarkReadButton";

function renderButton(overrides: Partial<React.ComponentProps<typeof MarkReadButton>> = {}) {
  const props: React.ComponentProps<typeof MarkReadButton> = {
    label: "Mark as read",
    loading: false,
    completed: false,
    className: "text-green-600",
    onClick: vi.fn(),
    ...overrides,
  };
  const utils = render(<MarkReadButton {...props} />);
  return { ...utils, props };
}

describe("MarkReadButton", () => {
  it("renders the label and forwards clicks", () => {
    const { props } = renderButton();

    const button = screen.getByRole("button", { name: "Mark as read" });
    fireEvent.click(button);

    expect(props.onClick).toHaveBeenCalledTimes(1);
  });

  it("disables the button while loading and shows a spinner", () => {
    const { container } = renderButton({ loading: true });

    const button = screen.getByRole("button");
    expect(button).toBeDisabled();
    expect(container.querySelector("svg")).toHaveClass("animate-spin");
  });

  it("shows the undo icon when the item is completed", () => {
    const { container } = renderButton({ completed: true });

    expect(container.querySelector("svg path")?.getAttribute("d")).toContain("M9 15");
  });

  it("shows the check icon when the item is not completed", () => {
    const { container } = renderButton({ completed: false });

    expect(container.querySelector("svg path")?.getAttribute("d")).toContain("M9 12.75");
  });

  it("shrinks the icon in compact mode", () => {
    const { container } = renderButton({ compact: true });

    expect(container.querySelector("svg")).toHaveClass("w-3.5", "h-3.5");
  });

  it("uses the regular icon size by default", () => {
    const { container } = renderButton();

    expect(container.querySelector("svg")).toHaveClass("w-4", "h-4");
  });
});
