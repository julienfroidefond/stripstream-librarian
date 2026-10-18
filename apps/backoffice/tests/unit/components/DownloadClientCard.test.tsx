import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { DownloadClientCard } from "@/app/(app)/settings/components/DownloadClientCard";

describe("DownloadClientCard", () => {
  it("saves the selected client", () => {
    const handleUpdateSetting = vi.fn().mockResolvedValue(undefined);
    render(
      <DownloadClientCard
        handleUpdateSetting={handleUpdateSetting}
        initialClient="qbittorrent"
        qbConfigured
        alldebridConfigured
      />
    );

    const radios = screen.getAllByRole("radio");
    expect(radios).toHaveLength(2);
    expect(radios[0]).toBeChecked();

    fireEvent.click(radios[1]);

    expect(handleUpdateSetting).toHaveBeenCalledWith("download_client", { client: "alldebrid" });
    expect(radios[1]).toBeChecked();
    expect(radios[0]).not.toBeChecked();
  });

  it("disables a client that is not configured", () => {
    const handleUpdateSetting = vi.fn();
    render(
      <DownloadClientCard
        handleUpdateSetting={handleUpdateSetting}
        initialClient="qbittorrent"
        qbConfigured
        alldebridConfigured={false}
      />
    );

    const radios = screen.getAllByRole("radio");
    expect(radios[1]).toBeDisabled();

    fireEvent.click(radios[1]);
    expect(handleUpdateSetting).not.toHaveBeenCalled();
  });

  it("preselects the first configured client when nothing is stored", () => {
    render(
      <DownloadClientCard
        handleUpdateSetting={vi.fn()}
        initialClient={null}
        qbConfigured={false}
        alldebridConfigured
      />
    );

    const radios = screen.getAllByRole("radio");
    expect(radios[1]).toBeChecked();
  });

  it("ignores a stored choice whose client is no longer configured", () => {
    render(
      <DownloadClientCard
        handleUpdateSetting={vi.fn()}
        initialClient="alldebrid"
        qbConfigured
        alldebridConfigured={false}
      />
    );

    // AllDebrid is stored but unconfigured, so snatches actually go to
    // qBittorrent: the card must show the same client as the runtime fallback.
    const radios = screen.getAllByRole("radio");
    expect(radios[0]).toBeChecked();
    expect(radios[1]).not.toBeChecked();
  });
});
