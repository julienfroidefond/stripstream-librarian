import { fireEvent, screen, waitFor } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { AlldebridCard, ALLDEBRID_LIMIT_RANGES, type AlldebridLimits } from "@/app/(app)/settings/components/AlldebridCard";
import { renderWithSettingsDraft } from "../helpers/settingsDraft";

function renderCard(
  initial?: Record<string, unknown>,
  limits: Partial<AlldebridLimits> = {},
) {
  return renderWithSettingsDraft(<AlldebridCard />, {
    initial: { ...initial, limits },
  });
}

/** The API key field is a password input (jsdom exposes no role for it). */
function apiKeyInput() {
  return document.querySelector('input[type="password"]') as HTMLInputElement;
}

function limitInput(key: keyof AlldebridLimits) {
  const id = key === "concurrent_alldebrid_downloads" ? "alldebrid-concurrency" : "alldebrid-stall-timeout";
  return document.getElementById(id) as HTMLInputElement;
}

const enabledSwitch = () => screen.getByRole("checkbox", { name: "settings.alldebridEnabled" });

describe("AlldebridCard", () => {
  it("renders empty defaults and a disabled test button", () => {
    renderCard();

    expect(screen.getByText("settings.alldebrid")).toBeInTheDocument();
    expect(apiKeyInput()).toHaveValue("");
    expect(apiKeyInput()).toHaveAttribute("placeholder", "settings.alldebridApiKeyPlaceholder");
    expect(enabledSwitch()).not.toBeChecked();
    expect(screen.getByRole("button", { name: "settings.testConnection" })).toBeDisabled();
  });

  it("never prefills the redacted key but reports it as configured", () => {
    renderCard({ alldebrid: { api_key: "********", enabled: true } });

    expect(apiKeyInput()).toHaveValue("");
    expect(apiKeyInput()).toHaveAttribute("placeholder", "settings.alldebridApiKeyConfigured");
    expect(enabledSwitch()).toBeChecked();
    expect(screen.getByRole("button", { name: "settings.testConnection" })).toBeEnabled();
  });

  it("saves a new key on blur and clears the field", async () => {
    const { writer } = renderCard();

    fireEvent.change(apiKeyInput(), { target: { value: "  secret  " } });
    fireEvent.blur(apiKeyInput());

    await waitFor(() => expect(writer).toHaveBeenCalledWith("alldebrid", {
      enabled: false,
      api_key: "secret",
    }));
    await waitFor(() => expect(apiKeyInput()).toHaveValue(""));
    expect(apiKeyInput()).toHaveAttribute("placeholder", "settings.alldebridApiKeyConfigured");
  });

  it("keeps the typed key when the write fails", async () => {
    const { writer } = renderCard(
      { alldebrid: { enabled: false } },
    );
    writer.mockRejectedValueOnce(new Error("upstream rejected the key"));

    fireEvent.change(apiKeyInput(), { target: { value: " secret " } });
    fireEvent.blur(apiKeyInput());

    await waitFor(() => expect(writer).toHaveBeenCalled());
    expect(apiKeyInput()).toHaveValue(" secret ");
    expect(apiKeyInput()).toHaveAttribute("placeholder", "settings.alldebridApiKeyPlaceholder");
  });

  it("adopts the redacted key returned by the server", async () => {
    const { writer } = renderCard();
    writer.mockResolvedValueOnce({ enabled: false, api_key: "********" });

    fireEvent.change(apiKeyInput(), { target: { value: "secret" } });
    fireEvent.blur(apiKeyInput());

    await waitFor(() => expect(apiKeyInput()).toHaveValue(""));
    expect(apiKeyInput()).toHaveAttribute("placeholder", "settings.alldebridApiKeyConfigured");
  });

  it("keeps the test button disabled when enabled but no key is stored", () => {
    renderCard({ alldebrid: { enabled: true } });

    expect(enabledSwitch()).toBeChecked();
    expect(screen.getByRole("button", { name: "settings.testConnection" })).toBeDisabled();
  });

  it("does not rewrite the section when an untouched key field blurs", async () => {
    const { writer } = renderCard({ alldebrid: { api_key: "********", enabled: true } });

    fireEvent.blur(apiKeyInput());
    await new Promise((resolve) => setTimeout(resolve, 0));

    expect(writer).not.toHaveBeenCalled();
  });

  it("saves the enabled toggle without touching the key", async () => {
    const { writer } = renderCard({ alldebrid: { api_key: "********", enabled: true } });

    fireEvent.click(enabledSwitch());

    await waitFor(() => expect(writer).toHaveBeenCalledWith("alldebrid", {
      enabled: false,
      api_key: "********",
    }));
    expect(apiKeyInput()).toHaveValue("");
    expect(enabledSwitch()).not.toBeChecked();
  });

  it("hides the API key from browser and password-manager autofill", () => {
    renderCard();
    const input = apiKeyInput();

    expect(input).toHaveAttribute("autocomplete", "off");
    expect(input).toHaveAttribute("data-lpignore", "true");
    expect(input).toHaveAttribute("data-1p-ignore");
    expect(input).toHaveAttribute("data-bwignore");
  });

  it("renders the AllDebrid limit defaults when none are stored", () => {
    renderCard();

    expect(limitInput("concurrent_alldebrid_downloads")).toHaveValue(
      ALLDEBRID_LIMIT_RANGES.concurrent_alldebrid_downloads.default
    );
    expect(limitInput("alldebrid_stall_timeout_minutes")).toHaveValue(
      ALLDEBRID_LIMIT_RANGES.alldebrid_stall_timeout_minutes.default
    );
  });

  it("prefills and saves the AllDebrid limits through the shared limits section", async () => {
    const { writer } = renderCard({}, {
      concurrent_alldebrid_downloads: 6,
      alldebrid_stall_timeout_minutes: 45,
    });

    expect(limitInput("concurrent_alldebrid_downloads")).toHaveValue(6);
    expect(limitInput("alldebrid_stall_timeout_minutes")).toHaveValue(45);

    fireEvent.change(limitInput("concurrent_alldebrid_downloads"), { target: { value: "3" } });
    fireEvent.blur(limitInput("concurrent_alldebrid_downloads"));

    await waitFor(() => expect(writer).toHaveBeenCalledWith(
      "limits",
      expect.objectContaining({ concurrent_alldebrid_downloads: 3 }),
    ));
    // A limits save must not touch the `alldebrid` setting (key/enabled).
    expect(writer).not.toHaveBeenCalledWith("alldebrid", expect.anything());
  });

  it("clamps the AllDebrid stall timeout and falls back when emptied", async () => {
    const { writer } = renderCard();

    fireEvent.change(limitInput("alldebrid_stall_timeout_minutes"), { target: { value: "999999" } });
    fireEvent.blur(limitInput("alldebrid_stall_timeout_minutes"));
    await waitFor(() => expect(writer).toHaveBeenLastCalledWith(
      "limits",
      expect.objectContaining({
        alldebrid_stall_timeout_minutes: ALLDEBRID_LIMIT_RANGES.alldebrid_stall_timeout_minutes.max,
      }),
    ));

    fireEvent.change(limitInput("alldebrid_stall_timeout_minutes"), { target: { value: "" } });
    fireEvent.blur(limitInput("alldebrid_stall_timeout_minutes"));
    await waitFor(() => expect(writer).toHaveBeenLastCalledWith(
      "limits",
      expect.objectContaining({
        alldebrid_stall_timeout_minutes: ALLDEBRID_LIMIT_RANGES.alldebrid_stall_timeout_minutes.default,
      }),
    ));
  });
});
