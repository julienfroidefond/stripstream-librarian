import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

vi.mock("@/app/components/FolderPicker", () => ({
  FolderPicker: ({ onSelect }: { onSelect: (path: string) => void }) => (
    <button type="button" onClick={() => onSelect("/library/foo")}>
      pick
    </button>
  ),
}));

import { LibraryForm } from "@/app/components/LibraryForm";

function renderForm() {
  const action = vi.fn();
  const { container } = render(<LibraryForm initialFolders={[]} action={action} />);
  return { action, container };
}

describe("LibraryForm", () => {
  it("renders the name field and a disabled submit button", () => {
    const { container } = renderForm();

    const name = screen.getByPlaceholderText("libraries.libraryName");
    expect(name).toBeRequired();
    expect(screen.getByRole("button", { name: "libraries.addButton" })).toBeDisabled();
    expect(container.querySelector('input[name="root_path"]')).toHaveValue("");
  });

  it("enables submit once a folder is selected and stores its path", () => {
    const { container } = renderForm();

    fireEvent.click(screen.getByRole("button", { name: "pick" }));

    expect(container.querySelector('input[name="root_path"]')).toHaveValue("/library/foo");
    expect(screen.getByRole("button", { name: "libraries.addButton" })).toBeEnabled();
  });

  it("submits the name and root path to the action", () => {
    const { action, container } = renderForm();

    fireEvent.change(screen.getByPlaceholderText("libraries.libraryName"), {
      target: { value: "My library" },
    });
    fireEvent.click(screen.getByRole("button", { name: "pick" }));
    fireEvent.submit(container.querySelector("form") as HTMLFormElement);

    expect(action).toHaveBeenCalledTimes(1);
    const formData = action.mock.calls[0][0] as FormData;
    expect(formData.get("name")).toBe("My library");
    expect(formData.get("root_path")).toBe("/library/foo");
  });
});
