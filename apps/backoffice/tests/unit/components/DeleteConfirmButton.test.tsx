import { fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { DeleteConfirmButton } from "@/app/components/DeleteConfirmButton";

const ENDPOINT = "/api/series/series-1";

function renderButton(overrides: Partial<React.ComponentProps<typeof DeleteConfirmButton>> = {}) {
  render(
    <DeleteConfirmButton
      endpoint={ENDPOINT}
      titleKey="seriesDetail.delete"
      confirmKey="seriesDetail.confirmDelete"
      labelKey="common.delete"
      {...overrides}
    />
  );
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("DeleteConfirmButton", () => {
  it("does not call the API until the deletion is confirmed", () => {
    const fetchMock = vi.fn().mockResolvedValue({ ok: true });
    vi.stubGlobal("fetch", fetchMock);
    renderButton();

    fireEvent.click(screen.getByRole("button", { name: "common.delete" }));

    expect(screen.getByText("seriesDetail.delete")).toBeInTheDocument();
    expect(screen.getByText("seriesDetail.confirmDelete")).toBeInTheDocument();
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it("closes the confirmation without deleting on cancel", () => {
    const fetchMock = vi.fn().mockResolvedValue({ ok: true });
    vi.stubGlobal("fetch", fetchMock);
    renderButton();

    fireEvent.click(screen.getByRole("button", { name: "common.delete" }));
    fireEvent.click(screen.getByRole("button", { name: "common.cancel" }));

    expect(screen.queryByText("seriesDetail.delete")).not.toBeInTheDocument();
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it("sends a DELETE request to the endpoint on confirm", async () => {
    const fetchMock = vi.fn().mockResolvedValue({ ok: true });
    vi.stubGlobal("fetch", fetchMock);
    renderButton();

    fireEvent.click(screen.getByRole("button", { name: "common.delete" }));
    const panel = screen.getByTestId("modal-panel");
    fireEvent.click(within(panel).getByRole("button", { name: "common.delete" }));

    expect(fetchMock).toHaveBeenCalledWith(ENDPOINT, { method: "DELETE" });
    await vi.waitFor(() =>
      expect(screen.queryByText("seriesDetail.delete")).not.toBeInTheDocument()
    );
  });

  it("supports a custom trigger through the render prop", () => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue({ ok: true }));
    renderButton({
      children: (open) => (
        <button type="button" onClick={open}>
          custom trigger
        </button>
      ),
    });

    expect(screen.queryByRole("button", { name: "common.delete" })).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "custom trigger" }));

    expect(screen.getByText("seriesDetail.delete")).toBeInTheDocument();
  });
});
