import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { AlldebridCard } from "@/app/(app)/settings/components/AlldebridCard";

function renderCard(
  initialData: Record<string, unknown> | null = null,
  onConfiguredChange?: (configured: boolean) => void
) {
  const handleUpdateSetting = vi.fn().mockResolvedValue(undefined);
  render(
    <AlldebridCard
      handleUpdateSetting={handleUpdateSetting}
      initialData={initialData}
      onConfiguredChange={onConfiguredChange}
    />
  );
  return { handleUpdateSetting };
}

/** The API key field is a password input (jsdom exposes no role for it). */
function apiKeyInput() {
  return document.querySelector('input[type="password"]') as HTMLInputElement;
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
    renderCard({ api_key: "********", enabled: true }, onConfiguredChange);

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
});
