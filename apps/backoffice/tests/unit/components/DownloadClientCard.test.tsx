import { fireEvent, screen, waitFor } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { DownloadClientCard } from "@/app/(app)/settings/components/DownloadClientCard";
import type { AlldebridSettings, QbittorrentSettings } from "@/lib/settings/sections";
import { useSettingsSection } from "@/app/(app)/settings/SettingsDraftProvider";
import { renderWithSettingsDraft } from "../helpers/settingsDraft";

function ConfigureQbittorrent({ patch }: { patch: Partial<QbittorrentSettings> }) {
  const section = useSettingsSection<QbittorrentSettings>("qbittorrent");
  return (
    <button type="button" onClick={() => section.patch(patch)}>
      configure-qbittorrent
    </button>
  );
}

function ConfigureAlldebrid({ patch }: { patch: Partial<AlldebridSettings> }) {
  const section = useSettingsSection<AlldebridSettings>("alldebrid");
  return (
    <button type="button" onClick={() => section.patch(patch)}>
      configure-alldebrid
    </button>
  );
}

describe("DownloadClientCard", () => {
  it("saves the selected client", async () => {
    const { writer } = renderWithSettingsDraft(<DownloadClientCard />, {
      initial: {
        download_client: { client: "qbittorrent" },
        qbittorrent: { url: "http://qb", username: "admin" },
        alldebrid: { enabled: true, api_key: "********" },
      },
    });

    const radios = screen.getAllByRole("radio");
    expect(radios).toHaveLength(2);
    expect(radios[0]).toBeChecked();

    fireEvent.click(radios[1]);

    await waitFor(() => expect(writer).toHaveBeenCalledWith("download_client", {
      client: "alldebrid",
    }));
    await waitFor(() => expect(radios[1]).toBeChecked());
    expect(radios[0]).not.toBeChecked();
  });

  it("disables a client that is not configured", () => {
    renderWithSettingsDraft(<DownloadClientCard />, {
      initial: {
        download_client: { client: "qbittorrent" },
        qbittorrent: { url: "http://qb", username: "admin" },
        alldebrid: { enabled: true, api_key: "" },
      },
    });

    const radios = screen.getAllByRole("radio");
    expect(radios[1]).toBeDisabled();

    fireEvent.click(radios[1]);
    expect(radios[0]).toBeChecked();
  });

  it("preselects the first configured client when nothing is stored", () => {
    renderWithSettingsDraft(<DownloadClientCard />, {
      initial: {
        qbittorrent: { enabled: false },
        alldebrid: { enabled: true, api_key: "********" },
      },
    });

    const radios = screen.getAllByRole("radio");
    expect(radios[1]).toBeChecked();
  });

  it("ignores a stored choice whose client is no longer configured", () => {
    renderWithSettingsDraft(<DownloadClientCard />, {
      initial: {
        download_client: { client: "alldebrid" },
        qbittorrent: { url: "http://qb", username: "admin" },
        alldebrid: { enabled: false, api_key: "********" },
      },
    });

    // AllDebrid is stored but unconfigured, so snatches actually go to
    // qBittorrent: the card must show the same client as the runtime fallback.
    const radios = screen.getAllByRole("radio");
    expect(radios[0]).toBeChecked();
    expect(radios[1]).not.toBeChecked();
  });

  it("reacts when a client is enabled/configured above", async () => {
    renderWithSettingsDraft(
      <>
        <DownloadClientCard />
        <ConfigureAlldebrid patch={{ enabled: true, api_key: "key" }} />
      </>,
      {
        initial: {
          qbittorrent: { enabled: false },
          alldebrid: { enabled: false, api_key: "" },
        },
      },
    );

    let radios = screen.getAllByRole("radio");
    expect(radios[0]).not.toBeChecked();
    expect(radios[1]).not.toBeChecked();
    expect(radios[1]).toBeDisabled();

    // AllDebrid gets configured elsewhere in the page: the card must pick it up
    // without remounting.
    fireEvent.click(screen.getByRole("button", { name: "configure-alldebrid" }));

    await waitFor(() => expect(screen.getAllByRole("radio")[1]).toBeEnabled());
    radios = screen.getAllByRole("radio");
    expect(radios[1]).toBeChecked();
  });

  it("falls back when the selected client is disabled above", async () => {
    renderWithSettingsDraft(
      <>
        <DownloadClientCard />
        <ConfigureQbittorrent patch={{ enabled: false }} />
        <ConfigureAlldebrid patch={{ enabled: true, api_key: "********" }} />
      </>,
      {
        initial: {
          download_client: { client: "qbittorrent" },
          qbittorrent: { url: "http://qb", username: "admin" },
          alldebrid: { enabled: true, api_key: "********" },
        },
      },
    );
    expect(screen.getAllByRole("radio")[0]).toBeChecked();

    fireEvent.click(screen.getByRole("button", { name: "configure-qbittorrent" }));

    await waitFor(() => expect(screen.getAllByRole("radio")[1]).toBeChecked());
  });
});
