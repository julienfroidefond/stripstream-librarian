import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { TagInput } from "@/app/components/ui/TagInput";

function renderTagInput(overrides: Partial<React.ComponentProps<typeof TagInput>> = {}) {
  const props: React.ComponentProps<typeof TagInput> = {
    values: [],
    input: "",
    onInputChange: vi.fn(),
    onAdd: vi.fn(),
    onRemove: vi.fn(),
    removeLabel: (value) => `Remove ${value}`,
    placeholder: "Add a tag",
    ...overrides,
  };
  render(<TagInput {...props} />);
  return props;
}

describe("TagInput", () => {
  it("renders every value as a pill", () => {
    renderTagInput({ values: ["action", "drama"] });

    expect(screen.getByText("action")).toBeInTheDocument();
    expect(screen.getByText("drama")).toBeInTheDocument();
  });

  it("calls onRemove with the pill index", () => {
    const props = renderTagInput({ values: ["action", "drama"] });

    fireEvent.click(screen.getByRole("button", { name: "Remove drama" }));

    expect(props.onRemove).toHaveBeenCalledWith(1);
  });

  it("reports input changes", () => {
    const props = renderTagInput();

    fireEvent.change(screen.getByPlaceholderText("Add a tag"), { target: { value: "fan" } });

    expect(props.onInputChange).toHaveBeenCalledWith("fan");
  });

  it("disables the add button while the input is empty or blank", () => {
    renderTagInput({ input: "   " });

    expect(screen.getByRole("button", { name: "+" })).toBeDisabled();
  });

  it("calls onAdd when the add button is clicked", () => {
    const props = renderTagInput({ input: "fan" });

    fireEvent.click(screen.getByRole("button", { name: "+" }));

    expect(props.onAdd).toHaveBeenCalledTimes(1);
  });

  it("disables every control when disabled", () => {
    renderTagInput({ values: ["action"], input: "fan", disabled: true });

    expect(screen.getByRole("button", { name: "+" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Remove action" })).toBeDisabled();
    expect(screen.getByPlaceholderText("Add a tag")).toBeDisabled();
  });

  it("uses the variant pill classes", () => {
    const { container } = render(
      <TagInput
        values={["action"]}
        input=""
        onInputChange={vi.fn()}
        onAdd={vi.fn()}
        onRemove={vi.fn()}
        removeLabel={(v) => v}
        placeholder="p"
        variant="success"
      />
    );

    expect(container.querySelector("span")).toHaveClass("bg-success/10", "text-success");
  });

  it("renders suggestions and extra slots", () => {
    renderTagInput({
      suggestions: <div>suggestions</div>,
      extra: <button type="button">extra</button>,
    });

    expect(screen.getByText("suggestions")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "extra" })).toBeInTheDocument();
  });
});
