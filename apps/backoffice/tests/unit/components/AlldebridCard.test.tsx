import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { AlldebridCard } from "@/app/(app)/settings/components/AlldebridCard";

function renderCard(initialData: Record<string, unknown> | null = null) {
  const handleUpdateSetting = vi.fn().mockResolvedValue(undefined);
  render(<AlldebridCard handleUpdateSetting={handleUpdateSetting} initialData={initialData} />);
  return { handleUpdateSetting };
}

/** The API key field is a password input (jsdom exposes no role for it). */
function apiKeyInput() {
  return document.querySelector('input[type="password"]') as HTMLInputElement;
}

describe("AlldebridCard", () => {
  it("renders empty defaults and a disabled test button", () => {
    renderCard();

    expect(screen.getByText("settings.alldebrid")).toBeInTheDocument();
    expect(apiKeyInput()).toHaveValue("");
    expect(apiKeyInput()).toHaveAttribute("placeholder", "settings.alldebridApiKeyPlaceholder");
    expect(screen.getByRole("combobox")).toHaveValue("false");
    expect(screen.getByRole("button", { name: "settings.testConnection" })).toBeDisabled();
  });

  it("never prefills the redacted key but reports it as configured", () => {
    renderCard({ api_key: "********", enabled: true });

    expect(apiKeyInput()).toHaveValue("");
    expect(apiKeyInput()).toHaveAttribute("placeholder", "settings.alldebridApiKeyConfigured");
    expect(screen.getByRole("combobox")).toHaveValue("true");
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

    expect(screen.getByRole("combobox")).toHaveValue("true");
    expect(screen.getByRole("button", { name: "settings.testConnection" })).toBeDisabled();
  });

  it("keeps the stored key when blurring an untouched field", () => {
    const { handleUpdateSetting } = renderCard({ api_key: "********", enabled: true });

    fireEvent.blur(apiKeyInput());

    expect(handleUpdateSetting).toHaveBeenCalledWith("alldebrid", { enabled: true });
  });

  it("saves the enabled toggle without touching the key", () => {
    const { handleUpdateSetting } = renderCard({ api_key: "********", enabled: true });

    fireEvent.change(screen.getByRole("combobox"), { target: { value: "false" } });

    expect(handleUpdateSetting).toHaveBeenCalledWith("alldebrid", { enabled: false });
    expect(apiKeyInput()).toHaveValue("");
  });
});
