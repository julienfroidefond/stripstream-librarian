import { fireEvent, screen, waitFor } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { ProwlarrCard } from "@/app/(app)/settings/components/ProwlarrCard";
import { renderWithSettingsDraft } from "../helpers/settingsDraft";

function renderCard(initial?: Record<string, unknown>) {
  return renderWithSettingsDraft(<ProwlarrCard />, { initial });
}

const enabledSwitch = () => screen.getByRole("checkbox", { name: "settings.prowlarrEnabled" });

describe("ProwlarrCard", () => {
  it("renders the title and default field values", () => {
    renderCard();

    expect(screen.getByText("settings.prowlarr")).toBeInTheDocument();
    expect(screen.getByText("settings.prowlarrDesc")).toBeInTheDocument();
    expect(screen.getByPlaceholderText("settings.prowlarrUrlPlaceholder")).toHaveValue("");
    expect(screen.getByPlaceholderText("7030, 7020")).toHaveValue("7030, 7020");
    expect(screen.getByRole("combobox")).toHaveValue("30");
    // Enabled by default.
    expect(enabledSwitch()).toBeChecked();
  });

  it("prefills from the stored section", () => {
    renderCard({
      prowlarr: {
        url: "http://prowlarr:9696",
        api_key: "secret",
        categories: [7030, 7020],
        rss_poll_interval_minutes: 60,
      },
    });

    expect(screen.getByPlaceholderText("settings.prowlarrUrlPlaceholder")).toHaveValue(
      "http://prowlarr:9696"
    );
    expect(screen.getByPlaceholderText("settings.prowlarrApiKeyPlaceholder")).toHaveValue("secret");
    expect(screen.getByPlaceholderText("7030, 7020")).toHaveValue("7030, 7020");
    expect(screen.getByRole("combobox")).toHaveValue("60");
    expect(enabledSwitch()).toBeChecked();
  });

  it("reflects a disabled integration from the stored section", () => {
    renderCard({ prowlarr: { url: "http://p", api_key: "k", enabled: false } });
    expect(enabledSwitch()).not.toBeChecked();
  });

  it("saves the URL on blur, keeping the parsed categories", async () => {
    const { writer } = renderCard({
      prowlarr: {
        url: "http://old:9696",
        api_key: "secret",
        categories: [11, 22],
        rss_poll_interval_minutes: 60,
      },
    });

    fireEvent.change(screen.getByPlaceholderText("settings.prowlarrUrlPlaceholder"), {
      target: { value: "http://new:9696" },
    });
    fireEvent.blur(screen.getByPlaceholderText("settings.prowlarrUrlPlaceholder"));

    await waitFor(() => expect(writer).toHaveBeenCalledWith("prowlarr", {
      url: "http://new:9696",
      api_key: "secret",
      categories: [11, 22],
      rss_poll_interval_minutes: 60,
      enabled: true,
    }));
  });

  it("saves the api key on blur", async () => {
    const { writer } = renderCard({
      prowlarr: { url: "http://prowlarr:9696", api_key: "" },
    });

    const key = screen.getByPlaceholderText("settings.prowlarrApiKeyPlaceholder");
    fireEvent.change(key, { target: { value: "secret" } });
    fireEvent.blur(key);

    await waitFor(() => expect(writer).toHaveBeenCalledWith(
      "prowlarr",
      expect.objectContaining({ api_key: "secret" })
    ));
  });

  it("saves the URL on blur while an already-enabled integration is incomplete", async () => {
    // Prowlarr defaults to enabled; the API key is still empty. The URL must
    // be persistable field by field, otherwise the first configuration is
    // impossible (each blur would wait for the sibling field).
    const { writer } = renderCard();

    fireEvent.change(screen.getByPlaceholderText("settings.prowlarrUrlPlaceholder"), {
      target: { value: "http://new:9696" },
    });
    fireEvent.blur(screen.getByPlaceholderText("settings.prowlarrUrlPlaceholder"));

    await waitFor(() => expect(writer).toHaveBeenCalledWith(
      "prowlarr",
      expect.objectContaining({ url: "http://new:9696", api_key: "", enabled: true }),
    ));
  });

  it("rejects enabling Prowlarr without a URL and API key", async () => {
    const { writer } = renderCard({ prowlarr: { enabled: false } });

    fireEvent.click(enabledSwitch());
    // Wait for the rejected optimistic edit to roll back, then assert no write.
    await waitFor(() => expect(enabledSwitch()).not.toBeChecked());

    expect(writer).not.toHaveBeenCalled();
  });

  it("drops invalid category entries", async () => {
    const { writer } = renderCard({
      prowlarr: { url: "http://p", api_key: "k" },
    });

    fireEvent.change(screen.getByPlaceholderText("7030, 7020"), {
      target: { value: "1, foo, 2" },
    });
    fireEvent.blur(screen.getByPlaceholderText("7030, 7020"));

    await waitFor(() => expect(writer).toHaveBeenCalledWith(
      "prowlarr",
      expect.objectContaining({ categories: [1, 2] })
    ));
  });

  it("saves when the RSS interval changes", async () => {
    const { writer } = renderCard({
      prowlarr: { url: "http://p", api_key: "k" },
    });

    fireEvent.change(screen.getByRole("combobox"), { target: { value: "1440" } });

    await waitFor(() => expect(writer).toHaveBeenCalledWith(
      "prowlarr",
      expect.objectContaining({ rss_poll_interval_minutes: 1440 })
    ));
  });

  it("saves the enabled toggle", async () => {
    const { writer } = renderCard({ prowlarr: { url: "http://p", api_key: "k" } });

    fireEvent.click(enabledSwitch());

    await waitFor(() => expect(writer).toHaveBeenCalledWith(
      "prowlarr",
      expect.objectContaining({ enabled: false })
    ));
    expect(enabledSwitch()).not.toBeChecked();
  });

  it("disables the test button until a URL and API key are set", () => {
    const { writer } = renderCard();
    const button = screen.getByRole("button", { name: "settings.testConnection" });
    expect(button).toBeDisabled();

    fireEvent.change(screen.getByPlaceholderText("settings.prowlarrUrlPlaceholder"), {
      target: { value: "http://p" },
    });
    fireEvent.change(screen.getByPlaceholderText("settings.prowlarrApiKeyPlaceholder"), {
      target: { value: "k" },
    });
    expect(button).toBeEnabled();
    expect(writer).not.toHaveBeenCalled();
  });

  it("disables the test button when the integration is switched off", () => {
    renderCard({ prowlarr: { url: "http://p", api_key: "k", enabled: false } });
    expect(screen.getByRole("button", { name: "settings.testConnection" })).toBeDisabled();
  });

  it("hides the credentials from browser and password-manager autofill", () => {
    renderCard();
    const url = screen.getByPlaceholderText("settings.prowlarrUrlPlaceholder");
    const apiKey = screen.getByPlaceholderText("settings.prowlarrApiKeyPlaceholder");

    for (const input of [url, apiKey]) {
      expect(input).toHaveAttribute("autocomplete", "off");
      expect(input).toHaveAttribute("data-lpignore", "true");
      expect(input).toHaveAttribute("data-1p-ignore");
      expect(input).toHaveAttribute("data-bwignore");
    }
  });
});
