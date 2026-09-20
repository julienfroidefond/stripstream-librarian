import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { RenameSeriesBooksModal } from "@/app/components/RenameSeriesBooksModal";

interface FetchCall {
  mode: string;
  overrides: { book_id: string; volume: number | null; volume_type: string | null }[];
}

function makeFetchMock() {
  const calls: FetchCall[] = [];
  const fetchMock = vi.fn(async (_url: string, init?: RequestInit) => {
    const body = JSON.parse((init?.body as string) ?? "{}");
    calls.push({ mode: body.mode, overrides: body.overrides ?? [] });
    const ov = (body.overrides ?? [])[0];
    const volume = ov?.volume ?? 4;
    const volumeType = ov?.volume_type ?? "regular";
    const newFilename = volumeType === "oneshot" ? "Frieren.cbz" : `Frieren - T0${volume}.cbz`;
    return {
      ok: true,
      status: 200,
      json: async () => ({
        series_id: "s1",
        renames: [
          {
            book_id: "b1",
            old_filename: "old.cbz",
            new_filename: newFilename,
            old_path: "/l/old.cbz",
            new_path: `/l/${newFilename}`,
            changed: true,
            volume,
            volume_type: volumeType,
          },
        ],
        errors: [],
        executed: body.mode === "execute",
      }),
    } as Response;
  });
  return { fetchMock, calls };
}

describe("RenameSeriesBooksModal", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("sends the selected volume type override when changed", async () => {
    const { fetchMock, calls } = makeFetchMock();
    vi.stubGlobal("fetch", fetchMock);

    render(<RenameSeriesBooksModal seriesId="s1" seriesName="Frieren" />);
    fireEvent.click(screen.getByText("rename.button"));

    const select = await screen.findByLabelText("rename.volumeType old.cbz");
    fireEvent.change(select, { target: { value: "oneshot" } });

    await waitFor(() => expect(calls.length).toBe(2));
    expect(calls[1]).toEqual({
      mode: "preview",
      overrides: [{ book_id: "b1", volume: null, volume_type: "oneshot" }],
    });
  });

  it("sends the corrected volume override on blur and on execute", async () => {
    const { fetchMock, calls } = makeFetchMock();
    vi.stubGlobal("fetch", fetchMock);

    render(<RenameSeriesBooksModal seriesId="s1" seriesName="Frieren" />);
    fireEvent.click(screen.getByText("rename.button"));

    const volumeInput = await screen.findByLabelText("rename.volume old.cbz");
    fireEvent.change(volumeInput, { target: { value: "5" } });
    fireEvent.blur(volumeInput);

    await waitFor(() => expect(calls.length).toBe(2));
    expect(calls[1].overrides).toEqual([{ book_id: "b1", volume: 5, volume_type: null }]);

    fireEvent.click(screen.getByText("rename.apply"));

    await waitFor(() => expect(calls.length).toBe(3));
    expect(calls[2]).toEqual({
      mode: "execute",
      overrides: [{ book_id: "b1", volume: 5, volume_type: null }],
    });
    expect(await screen.findByText(/rename\.success/)).toBeInTheDocument();
  });

  it("disables the volume input for one-shot books", async () => {
    const { fetchMock } = makeFetchMock();
    vi.stubGlobal("fetch", fetchMock);

    render(<RenameSeriesBooksModal seriesId="s1" seriesName="Frieren" />);
    fireEvent.click(screen.getByText("rename.button"));

    const select = await screen.findByLabelText("rename.volumeType old.cbz");
    fireEvent.change(select, { target: { value: "oneshot" } });

    await waitFor(() =>
      expect(screen.getByLabelText("rename.volume old.cbz")).toBeDisabled(),
    );
  });
});
