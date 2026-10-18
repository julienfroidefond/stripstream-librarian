import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { AlldebridCard, ALLDEBRID_LIMIT_RANGES, type AlldebridLimits } from "@/app/(app)/settings/components/AlldebridCard";

function renderCard(
  initialData: Record<string, unknown> | null = null,
  limits: Partial<AlldebridLimits> = {},
  onConfiguredChange?: (configured: boolean) => void
) {
  const handleUpdateSetting = vi.fn().mockResolvedValue(undefined);
  const onSaveLimits = vi.fn();
  render(
    <AlldebridCard
      handleUpdateSetting={handleUpdateSetting}
      initialData={initialData}
      limits={limits}
      onSaveLimits={onSaveLimits}
      onConfiguredChange={onConfiguredChange}
    />
  );
  return { handleUpdateSetting, onSaveLimits };
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
    renderCard({ api_key: "********", enabled: true });

    expect(apiKeyInput()).toHaveValue("");
    expect(apiKeyInput()).toHaveAttribute("placeholder", "settings.alldebridApiKeyConfigured");
    expect(enabledSwitch()).toBeChecked();
    expect(screen.getByRole("button", { name: "settings.testConnection" })).toBeEnabled();
  });

  it("saves a new key on blur and clears the field", () => {
    const { handleUpdateSetting } = renderCard();

    fireEvent.change(apiKeyInput(), { target: { value: "  secret  " } });
    fireEvent.blur(apiKeyInput());

    expect(handleUpdateSetting).toHaveBeenCalledWith("alldebrid", {
      enabled: false,
      api_key: "secret",
    });
    expect(apiKeyInput()).toHaveValue("");
    expect(apiKeyInput()).toHaveAttribute("placeholder", "settings.alldebridApiKeyConfigured");
  });

  it("keeps the test button disabled when enabled but no key is stored", () => {
    renderCard({ enabled: true });

    expect(enabledSwitch()).toBeChecked();
    expect(screen.getByRole("button", { name: "settings.testConnection" })).toBeDisabled();
  });

  it("keeps the stored key when blurring an untouched field", () => {
    const { handleUpdateSetting } = renderCard({ api_key: "********", enabled: true });

    fireEvent.blur(apiKeyInput());

    expect(handleUpdateSetting).toHaveBeenCalledWith("alldebrid", { enabled: true });
  });

  it("saves the enabled toggle without touching the key", () => {
    const { handleUpdateSetting } = renderCard({ api_key: "********", enabled: true });

    fireEvent.click(enabledSwitch());

    expect(handleUpdateSetting).toHaveBeenCalledWith("alldebrid", { enabled: false });
    expect(apiKeyInput()).toHaveValue("");
    expect(enabledSwitch()).not.toBeChecked();
  });

  it("reports its usability to the parent (enabled and keyed)", () => {
    const onConfiguredChange = vi.fn();
    renderCard({ api_key: "********", enabled: true }, {}, onConfiguredChange);

    expect(onConfiguredChange).toHaveBeenLastCalledWith(true);

    fireEvent.click(enabledSwitch());
    expect(onConfiguredChange).toHaveBeenLastCalledWith(false);
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

  it("prefills and saves the AllDebrid limits through onSaveLimits", () => {
    const { onSaveLimits, handleUpdateSetting } = renderCard(null, {
      concurrent_alldebrid_downloads: 6,
      alldebrid_stall_timeout_minutes: 45,
    });

    expect(limitInput("concurrent_alldebrid_downloads")).toHaveValue(6);
    expect(limitInput("alldebrid_stall_timeout_minutes")).toHaveValue(45);

    fireEvent.change(limitInput("concurrent_alldebrid_downloads"), { target: { value: "3" } });
    fireEvent.blur(limitInput("concurrent_alldebrid_downloads"));

    expect(onSaveLimits).toHaveBeenCalledWith({ concurrent_alldebrid_downloads: 3 });
    // A limits save must not touch the `alldebrid` setting (key/enabled).
    expect(handleUpdateSetting).not.toHaveBeenCalled();
  });

  it("clamps the AllDebrid stall timeout and falls back when emptied", () => {
    const { onSaveLimits } = renderCard();

    fireEvent.change(limitInput("alldebrid_stall_timeout_minutes"), { target: { value: "999999" } });
    fireEvent.blur(limitInput("alldebrid_stall_timeout_minutes"));
    expect(onSaveLimits).toHaveBeenLastCalledWith({
      alldebrid_stall_timeout_minutes: ALLDEBRID_LIMIT_RANGES.alldebrid_stall_timeout_minutes.max,
    });

    fireEvent.change(limitInput("alldebrid_stall_timeout_minutes"), { target: { value: "" } });
    fireEvent.blur(limitInput("alldebrid_stall_timeout_minutes"));
    expect(onSaveLimits).toHaveBeenLastCalledWith({
      alldebrid_stall_timeout_minutes: ALLDEBRID_LIMIT_RANGES.alldebrid_stall_timeout_minutes.default,
    });
  });
});
