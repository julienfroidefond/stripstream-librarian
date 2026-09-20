import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { RenameFormatCard } from "@/app/(app)/settings/components/RenameFormatCard";

describe("RenameFormatCard", () => {
  it("renders default templates and their previews", () => {
    render(<RenameFormatCard handleUpdateSetting={vi.fn()} initialRenameFormat={null} initialRenameFormatHs={null} />);
    expect(screen.getByDisplayValue("{series_name} - T{volume_padded} - {title}")).toBeInTheDocument();
    expect(screen.getByDisplayValue("{series_name} - HS {volume_padded}")).toBeInTheDocument();
    expect(screen.getByDisplayValue("{series_name} - INT {volume_padded}")).toBeInTheDocument();
    expect(screen.getByDisplayValue("{series_name}")).toBeInTheDocument();
    expect(screen.getByText("Dragon Ball - T01 - Son Goku et ses amis.cbz")).toBeInTheDocument();
    expect(screen.getByText("Dragon Ball - HS 01.cbz")).toBeInTheDocument();
    expect(screen.getByText("Dragon Ball - INT 01.cbz")).toBeInTheDocument();
    expect(screen.getByText("Dragon Ball.cbz")).toBeInTheDocument();
  });

  it("saves the regular template on blur", async () => {
    const handleUpdateSetting = vi.fn().mockResolvedValue(undefined);
    render(<RenameFormatCard handleUpdateSetting={handleUpdateSetting} initialRenameFormat="{series_name}" initialRenameFormatHs={null} />);
    const input = screen.getByLabelText("rename.template");
    fireEvent.change(input, { target: { value: "{title}" } });
    fireEvent.blur(input);
    await waitFor(() => expect(handleUpdateSetting).toHaveBeenCalledWith("rename_format", "{title}"));
  });

  it("saves the HS template on blur", async () => {
    const handleUpdateSetting = vi.fn().mockResolvedValue(undefined);
    render(<RenameFormatCard handleUpdateSetting={handleUpdateSetting} initialRenameFormat={null} initialRenameFormatHs="{series_name}" />);
    const input = screen.getByLabelText("rename.templateHs");
    fireEvent.change(input, { target: { value: "{isbn}" } });
    fireEvent.blur(input);
    await waitFor(() => expect(handleUpdateSetting).toHaveBeenCalledWith("rename_format_hs", "{isbn}"));
  });

  it("saves the integral template on blur", async () => {
    const handleUpdateSetting = vi.fn().mockResolvedValue(undefined);
    render(<RenameFormatCard handleUpdateSetting={handleUpdateSetting} initialRenameFormat={null} initialRenameFormatHs={null} initialRenameFormatInt="{series_name}" />);
    const input = screen.getByLabelText("rename.templateInt");
    fireEvent.change(input, { target: { value: "{volume}" } });
    fireEvent.blur(input);
    await waitFor(() => expect(handleUpdateSetting).toHaveBeenCalledWith("rename_format_int", "{volume}"));
  });

  it("saves the oneshot template on blur", async () => {
    const handleUpdateSetting = vi.fn().mockResolvedValue(undefined);
    render(<RenameFormatCard handleUpdateSetting={handleUpdateSetting} initialRenameFormat={null} initialRenameFormatHs={null} initialRenameFormatOneshot="{series_name}" />);
    const input = screen.getByLabelText("rename.templateOneshot");
    fireEvent.change(input, { target: { value: "{title}" } });
    fireEvent.blur(input);
    await waitFor(() => expect(handleUpdateSetting).toHaveBeenCalledWith("rename_format_oneshot", "{title}"));
  });

  it("appends a variable to the active template when clicked", async () => {
    render(<RenameFormatCard handleUpdateSetting={vi.fn()} initialRenameFormat="{series_name}" initialRenameFormatHs={null} />);
    await userEvent.click(screen.getByText("{volume}"));
    expect(screen.getByLabelText("rename.template")).toHaveValue("{series_name}{volume}");
    expect(screen.getByText("Dragon Ball1.cbz")).toBeInTheDocument();
  });

  it("cleans up dangling separators around unknown variables", async () => {
    render(<RenameFormatCard handleUpdateSetting={vi.fn()} initialRenameFormat="{series_name} - {unknown}" initialRenameFormatHs="{unknown} - {volume}" initialRenameFormatOneshot="{title}" />);
    expect(screen.getByText("Dragon Ball.cbz")).toBeInTheDocument();
    expect(screen.getByText("1.cbz")).toBeInTheDocument();
  });
});
