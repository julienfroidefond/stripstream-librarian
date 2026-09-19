import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { QBittorrentCard } from "@/app/(app)/settings/components/QBittorrentCard";

function renderCard(
  initialQbittorrent: Record<string, unknown> | null = null,
  initialTorrentImport: Record<string, unknown> | null = null
) {
  const handleUpdateSetting = vi.fn().mockResolvedValue(undefined);
  render(
    <QBittorrentCard
      handleUpdateSetting={handleUpdateSetting}
      initialQbittorrent={initialQbittorrent}
      initialTorrentImport={initialTorrentImport}
    />
  );
  return { handleUpdateSetting };
}

describe("QBittorrentCard", () => {
  it("renders the title and empty defaults", () => {
    renderCard();

    expect(screen.getByText("settings.qbittorrent")).toBeInTheDocument();
    expect(screen.getByPlaceholderText("settings.qbittorrentUrlPlaceholder")).toHaveValue("");
    expect(screen.getByRole("combobox")).toHaveValue("false");
  });

  it("prefills the connection details", () => {
    renderCard({ url: "http://qb:8080", username: "admin", password: "pw" }, { enabled: true });

    expect(screen.getByPlaceholderText("settings.qbittorrentUrlPlaceholder")).toHaveValue(
      "http://qb:8080"
    );
    const inputs = screen.getAllByDisplayValue("admin");
    expect(inputs).toHaveLength(1);
    expect(screen.getByDisplayValue("pw")).toBeInTheDocument();
    expect(screen.getByRole("combobox")).toHaveValue("true");
  });

  it("saves the connection on blur", () => {
    const { handleUpdateSetting } = renderCard();

    fireEvent.change(screen.getByPlaceholderText("settings.qbittorrentUrlPlaceholder"), {
      target: { value: "http://qb:8080" },
    });
    fireEvent.blur(screen.getByPlaceholderText("settings.qbittorrentUrlPlaceholder"));

    expect(handleUpdateSetting).toHaveBeenCalledWith("qbittorrent", {
      url: "http://qb:8080",
      username: "",
      password: "",
    });
  });

  it("saves the torrent import toggle and reveals the polling info", () => {
    const { handleUpdateSetting } = renderCard();

    expect(screen.queryByText("settings.torrentImportPollingInfo")).toBeNull();

    fireEvent.change(screen.getByRole("combobox"), { target: { value: "true" } });

    expect(handleUpdateSetting).toHaveBeenCalledWith("torrent_import", { enabled: true });
    expect(screen.getByText("settings.torrentImportPollingInfo")).toBeInTheDocument();

    fireEvent.change(screen.getByRole("combobox"), { target: { value: "false" } });
    expect(handleUpdateSetting).toHaveBeenLastCalledWith("torrent_import", { enabled: false });
    expect(screen.queryByText("settings.torrentImportPollingInfo")).toBeNull();
  });

  it("disables the test button until a URL and username are set", () => {
    renderCard();
    const button = screen.getByRole("button", { name: "settings.testConnection" });
    expect(button).toBeDisabled();

    fireEvent.change(screen.getByPlaceholderText("settings.qbittorrentUrlPlaceholder"), {
      target: { value: "http://qb" },
    });
    fireEvent.change(screen.getAllByRole("textbox")[1], { target: { value: "admin" } });

    expect(button).toBeEnabled();
  });
});
