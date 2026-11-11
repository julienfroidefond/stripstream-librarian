import { fireEvent, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";

import { RenameFormatCard } from "@/app/(app)/settings/components/RenameFormatCard";
import { renderWithSettingsDraft } from "../../helpers/settingsDraft";

function renderCard(initial?: Record<string, unknown>) {
  return renderWithSettingsDraft(<RenameFormatCard />, { initial });
}

describe("RenameFormatCard", () => {
  it("renders default templates and their previews", () => {
    renderCard();
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
    const { writer } = renderCard({ rename_format: "{series_name}" });
    const input = screen.getByLabelText("rename.template");
    fireEvent.change(input, { target: { value: "{title}" } });
    fireEvent.blur(input);
    await waitFor(() => expect(writer).toHaveBeenCalledWith("rename_format", "{title}"));
  });

  it("saves the HS template on blur", async () => {
    const { writer } = renderCard({ rename_format_hs: "{series_name}" });
    const input = screen.getByLabelText("rename.templateHs");
    fireEvent.change(input, { target: { value: "{isbn}" } });
    fireEvent.blur(input);
    await waitFor(() => expect(writer).toHaveBeenCalledWith("rename_format_hs", "{isbn}"));
  });

  it("saves the integral template on blur", async () => {
    const { writer } = renderCard({ rename_format_int: "{series_name}" });
    const input = screen.getByLabelText("rename.templateInt");
    fireEvent.change(input, { target: { value: "{volume}" } });
    fireEvent.blur(input);
    await waitFor(() => expect(writer).toHaveBeenCalledWith("rename_format_int", "{volume}"));
  });

  it("saves the oneshot template on blur", async () => {
    const { writer } = renderCard({ rename_format_oneshot: "{series_name}" });
    const input = screen.getByLabelText("rename.templateOneshot");
    fireEvent.change(input, { target: { value: "{title}" } });
    fireEvent.blur(input);
    await waitFor(() => expect(writer).toHaveBeenCalledWith("rename_format_oneshot", "{title}"));
  });

  it("appends a variable to the active template when clicked", async () => {
    renderCard({ rename_format: "{series_name}" });
    await userEvent.click(screen.getByText("{volume}"));
    expect(screen.getByLabelText("rename.template")).toHaveValue("{series_name}{volume}");
    expect(screen.getByText("Dragon Ball1.cbz")).toBeInTheDocument();
  });

  it("cleans up dangling separators around unknown variables", () => {
    renderCard({
      rename_format: "{series_name} - {unknown}",
      rename_format_hs: "{unknown} - {volume}",
      rename_format_oneshot: "{title}",
    });
    expect(screen.getByText("Dragon Ball.cbz")).toBeInTheDocument();
    expect(screen.getByText("1.cbz")).toBeInTheDocument();
  });
});
