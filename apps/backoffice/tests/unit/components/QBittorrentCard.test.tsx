import { fireEvent, screen, waitFor } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { QBittorrentCard } from "@/app/(app)/settings/components/QBittorrentCard";
import { renderWithSettingsDraft } from "../helpers/settingsDraft";

function renderCard(initial?: Record<string, unknown>) {
  return renderWithSettingsDraft(<QBittorrentCard />, { initial });
}

const enabledSwitch = () => screen.getByRole("checkbox", { name: "settings.qbittorrentEnabled" });

describe("QBittorrentCard", () => {
  it("renders the title and empty defaults", () => {
    renderCard();

    expect(screen.getByText("settings.qbittorrent")).toBeInTheDocument();
    expect(screen.getByPlaceholderText("settings.qbittorrentUrlPlaceholder")).toHaveValue("");
    expect(enabledSwitch()).toBeChecked();
    expect(screen.queryByRole("checkbox", { name: "settings.torrentImportEnabled" })).not.toBeInTheDocument();
  });

  it("prefills the connection details", () => {
    renderCard({
      qbittorrent: { url: "http://qb:8080", username: "admin", password: "pw", enabled: true },
    });

    expect(screen.getByPlaceholderText("settings.qbittorrentUrlPlaceholder")).toHaveValue(
      "http://qb:8080"
    );
    const inputs = screen.getAllByDisplayValue("admin");
    expect(inputs).toHaveLength(1);
    expect(screen.getByDisplayValue("pw")).toBeInTheDocument();
    expect(enabledSwitch()).toBeChecked();
  });

  it("reflects a disabled client from the stored section", () => {
    renderCard({ qbittorrent: { url: "http://qb", username: "admin", password: "pw", enabled: false } });
    expect(enabledSwitch()).not.toBeChecked();
  });

  it("saves the connection on blur", async () => {
    const { writer } = renderCard({
      qbittorrent: { url: "http://old:8080", username: "admin", password: "", enabled: true },
    });

    fireEvent.change(screen.getByPlaceholderText("settings.qbittorrentUrlPlaceholder"), {
      target: { value: "http://qb:8080" },
    });
    fireEvent.blur(screen.getByPlaceholderText("settings.qbittorrentUrlPlaceholder"));

    await waitFor(() => expect(writer).toHaveBeenCalledWith("qbittorrent", {
      url: "http://qb:8080",
      username: "admin",
      password: "",
      enabled: true,
    }));
  });

  it("saves the username and password on blur", async () => {
    const { writer } = renderCard({
      qbittorrent: { url: "http://qb", username: "", password: "", enabled: true },
    });

    const username = screen.getAllByRole("textbox")[1];
    fireEvent.change(username, { target: { value: "admin" } });
    fireEvent.blur(username);
    await waitFor(() => expect(writer).toHaveBeenLastCalledWith("qbittorrent", {
      url: "http://qb",
      username: "admin",
      password: "",
      enabled: true,
    }));

    const password = document.querySelector('input[type="password"]') as HTMLInputElement;
    fireEvent.change(password, { target: { value: "pw" } });
    fireEvent.blur(password);
    await waitFor(() => expect(writer).toHaveBeenLastCalledWith("qbittorrent", {
      url: "http://qb",
      username: "admin",
      password: "pw",
      enabled: true,
    }));
  });

  it("saves a field while default-enabled and connection fields are incomplete", async () => {
    const { writer } = renderCard();

    fireEvent.change(screen.getByPlaceholderText("settings.qbittorrentUrlPlaceholder"), {
      target: { value: "http://qb" },
    });
    fireEvent.blur(screen.getByPlaceholderText("settings.qbittorrentUrlPlaceholder"));

    await waitFor(() => expect(writer).toHaveBeenCalledWith("qbittorrent", {
      url: "http://qb",
      username: "",
      password: "",
      enabled: true,
    }));
    expect(enabledSwitch()).toBeChecked();
  });

  it("does not enable a disabled client without its connection fields", async () => {
    const { writer } = renderCard({ qbittorrent: { enabled: false } });

    fireEvent.click(enabledSwitch());
    await waitFor(() => expect(enabledSwitch()).not.toBeChecked());

    expect(writer).not.toHaveBeenCalled();
  });

  it("saves the enabled toggle", async () => {
    const { writer } = renderCard({
      qbittorrent: { url: "http://qb", username: "admin", password: "pw" },
    });

    fireEvent.click(enabledSwitch());

    await waitFor(() => expect(writer).toHaveBeenCalledWith("qbittorrent", {
      url: "http://qb",
      username: "admin",
      password: "pw",
      enabled: false,
    }));
    expect(enabledSwitch()).not.toBeChecked();
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
    renderCard({ qbittorrent: { url: "http://qb", username: "admin", password: "pw", enabled: false } });
    expect(screen.getByRole("button", { name: "settings.testConnection" })).toBeDisabled();
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
