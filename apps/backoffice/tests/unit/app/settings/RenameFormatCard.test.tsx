import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { RenameFormatCard } from "@/app/(app)/settings/components/RenameFormatCard";

describe("RenameFormatCard", () => {
  it("renders default templates and their previews", () => {
    render(<RenameFormatCard handleUpdateSetting={vi.fn()} initialRenameFormat={null} initialRenameFormatHs={null} />);
    expect(screen.getByDisplayValue("{series_name} - T{volume_padded} - {title}")).toBeInTheDocument();
    expect(screen.getByDisplayValue("{series_name} - HS {volume_padded}")).toBeInTheDocument();
    expect(screen.getByText("Dragon Ball - T01 - Son Goku et ses amis.cbz")).toBeInTheDocument();
    expect(screen.getByText("Dragon Ball - HS 01.cbz")).toBeInTheDocument();
  });

  it("saves the template on blur", async () => {
    const handleUpdateSetting = vi.fn().mockResolvedValue(undefined);
    render(<RenameFormatCard handleUpdateSetting={handleUpdateSetting} initialRenameFormat="{series_name}" initialRenameFormatHs={null} />);
    const input = screen.getByDisplayValue("{series_name}");
    fireEvent.change(input, { target: { value: "{title}" } });
    fireEvent.blur(input);
    await waitFor(() => expect(handleUpdateSetting).toHaveBeenCalledWith("rename_format", "{title}"));
  });

  it("saves the HS template on blur", async () => {
    const handleUpdateSetting = vi.fn().mockResolvedValue(undefined);
    render(<RenameFormatCard handleUpdateSetting={handleUpdateSetting} initialRenameFormat={null} initialRenameFormatHs="{series_name}" />);
    const input = screen.getByDisplayValue("{series_name}");
    fireEvent.change(input, { target: { value: "{isbn}" } });
    fireEvent.blur(input);
    await waitFor(() => expect(handleUpdateSetting).toHaveBeenCalledWith("rename_format_hs", "{isbn}"));
  });

  it("appends a variable to the template when clicked", async () => {
    render(<RenameFormatCard handleUpdateSetting={vi.fn()} initialRenameFormat="{series_name}" initialRenameFormatHs={null} />);
    await userEvent.click(screen.getByText("{volume}"));
    expect(screen.getByDisplayValue("{series_name}{volume}")).toBeInTheDocument();
    expect(screen.getByText("Dragon Ball1.cbz")).toBeInTheDocument();
  });

  it("cleans up dangling separators around unknown variables", async () => {
    render(<RenameFormatCard handleUpdateSetting={vi.fn()} initialRenameFormat="{series_name} - {unknown}" initialRenameFormatHs="{unknown} - {volume}" />);
    expect(screen.getByText("Dragon Ball.cbz")).toBeInTheDocument();
    expect(screen.getByText("1.cbz")).toBeInTheDocument();
  });
});
