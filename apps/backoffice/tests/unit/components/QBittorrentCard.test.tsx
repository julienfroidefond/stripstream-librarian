import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { QBittorrentCard } from "@/app/(app)/settings/components/QBittorrentCard";

function renderCard(
  initialQbittorrent: Record<string, unknown> | null = null,
  initialTorrentImport: Record<string, unknown> | null = null,
  onConfiguredChange?: (configured: boolean) => void
) {
  const handleUpdateSetting = vi.fn().mockResolvedValue(undefined);
  render(
    <QBittorrentCard
      handleUpdateSetting={handleUpdateSetting}
      initialQbittorrent={initialQbittorrent}
      initialTorrentImport={initialTorrentImport}
      onConfiguredChange={onConfiguredChange}
    />
  );
  return { handleUpdateSetting };
}

const enabledSwitch = () => screen.getByRole("checkbox", { name: "settings.qbittorrentEnabled" });
const importSwitch = () => screen.getByRole("checkbox", { name: "settings.torrentImportEnabled" });

describe("QBittorrentCard", () => {
  it("renders the title and empty defaults", () => {
    renderCard();

    expect(screen.getByText("settings.qbittorrent")).toBeInTheDocument();
    expect(screen.getByPlaceholderText("settings.qbittorrentUrlPlaceholder")).toHaveValue("");
    // Enabled by default, auto-import off.
    expect(enabledSwitch()).toBeChecked();
    expect(importSwitch()).not.toBeChecked();
  });

  it("prefills the connection details", () => {
    renderCard(
      { url: "http://qb:8080", username: "admin", password: "pw", enabled: true },
      { enabled: true }
    );

    expect(screen.getByPlaceholderText("settings.qbittorrentUrlPlaceholder")).toHaveValue(
      "http://qb:8080"
    );
    const inputs = screen.getAllByDisplayValue("admin");
    expect(inputs).toHaveLength(1);
    expect(screen.getByDisplayValue("pw")).toBeInTheDocument();
    expect(enabledSwitch()).toBeChecked();
    expect(importSwitch()).toBeChecked();
  });

  it("reflects a disabled client from the initial data", () => {
    renderCard({ url: "http://qb", username: "admin", password: "pw", enabled: false });
    expect(enabledSwitch()).not.toBeChecked();
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
      enabled: true,
    });
  });

  it("saves the username and password on blur", () => {
    const { handleUpdateSetting } = renderCard();

    const username = screen.getAllByRole("textbox")[1];
    fireEvent.change(username, { target: { value: "admin" } });
    fireEvent.blur(username);
    expect(handleUpdateSetting).toHaveBeenLastCalledWith("qbittorrent", {
      url: "",
      username: "admin",
      password: "",
      enabled: true,
    });

    const password = document.querySelector('input[type="password"]') as HTMLInputElement;
    fireEvent.change(password, { target: { value: "pw" } });
    fireEvent.blur(password);
    expect(handleUpdateSetting).toHaveBeenLastCalledWith("qbittorrent", {
      url: "",
      username: "admin",
      password: "pw",
      enabled: true,
    });
  });

  it("saves the enabled toggle", () => {
    const { handleUpdateSetting } = renderCard({
      url: "http://qb",
      username: "admin",
      password: "pw",
    });

    fireEvent.click(enabledSwitch());

    expect(handleUpdateSetting).toHaveBeenCalledWith("qbittorrent", {
      url: "http://qb",
      username: "admin",
      password: "pw",
      enabled: false,
    });
    expect(enabledSwitch()).not.toBeChecked();
  });

  it("saves the torrent import toggle and reveals the polling info", () => {
    const { handleUpdateSetting } = renderCard();

    expect(screen.queryByText("settings.torrentImportPollingInfo")).toBeNull();

    fireEvent.click(importSwitch());

    expect(handleUpdateSetting).toHaveBeenCalledWith("torrent_import", { enabled: true });
    expect(screen.getByText("settings.torrentImportPollingInfo")).toBeInTheDocument();

    fireEvent.click(importSwitch());
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

  it("disables the test button when the client is switched off", () => {
    renderCard({ url: "http://qb", username: "admin", password: "pw", enabled: false });
    expect(screen.getByRole("button", { name: "settings.testConnection" })).toBeDisabled();
  });

  it("reports its usability to the parent", () => {
    const onConfiguredChange = vi.fn();
    renderCard(null, null, onConfiguredChange);

    expect(onConfiguredChange).toHaveBeenLastCalledWith(false);

    fireEvent.change(screen.getByPlaceholderText("settings.qbittorrentUrlPlaceholder"), {
      target: { value: "http://qb" },
    });
    fireEvent.change(screen.getAllByRole("textbox")[1], { target: { value: "admin" } });

    expect(onConfiguredChange).toHaveBeenLastCalledWith(true);
  });

  it("hides the credentials from browser and password-manager autofill", () => {
    renderCard();
    const url = screen.getByPlaceholderText("settings.qbittorrentUrlPlaceholder");
    const username = screen.getAllByRole("textbox")[1];
    const password = document.querySelector('input[type="password"]') as HTMLInputElement;

    for (const input of [url, username, password]) {
      expect(input).toHaveAttribute("autocomplete", "off");
      expect(input).toHaveAttribute("data-lpignore", "true");
      expect(input).toHaveAttribute("data-1p-ignore");
      expect(input).toHaveAttribute("data-bwignore");
    }
  });
});
