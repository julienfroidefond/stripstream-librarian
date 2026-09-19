import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { ProwlarrCard } from "@/app/(app)/settings/components/ProwlarrCard";

function renderCard(initialData: Record<string, unknown> | null = null) {
  const handleUpdateSetting = vi.fn().mockResolvedValue(undefined);
  render(<ProwlarrCard handleUpdateSetting={handleUpdateSetting} initialData={initialData} />);
  return { handleUpdateSetting };
}

describe("ProwlarrCard", () => {
  it("renders the title and default field values", () => {
    renderCard();

    expect(screen.getByText("settings.prowlarr")).toBeInTheDocument();
    expect(screen.getByText("settings.prowlarrDesc")).toBeInTheDocument();
    expect(screen.getByPlaceholderText("settings.prowlarrUrlPlaceholder")).toHaveValue("");
    expect(screen.getByPlaceholderText("7030, 7020")).toHaveValue("7030, 7020");
    expect(screen.getByRole("combobox")).toHaveValue("30");
  });

  it("prefills from the initial data", () => {
    renderCard({
      url: "http://prowlarr:9696",
      api_key: "secret",
      categories: [7030, 7020],
      rss_poll_interval_minutes: 60,
    });

    expect(screen.getByPlaceholderText("settings.prowlarrUrlPlaceholder")).toHaveValue(
      "http://prowlarr:9696"
    );
    expect(screen.getByPlaceholderText("settings.prowlarrApiKeyPlaceholder")).toHaveValue("secret");
    expect(screen.getByPlaceholderText("7030, 7020")).toHaveValue("7030, 7020");
    expect(screen.getByRole("combobox")).toHaveValue("60");
  });

  it("saves the URL on blur, keeping the parsed categories", () => {
    const { handleUpdateSetting } = renderCard({
      categories: [11, 22],
      rss_poll_interval_minutes: 60,
    });

    fireEvent.change(screen.getByPlaceholderText("settings.prowlarrUrlPlaceholder"), {
      target: { value: "http://new:9696" },
    });
    fireEvent.blur(screen.getByPlaceholderText("settings.prowlarrUrlPlaceholder"));

    expect(handleUpdateSetting).toHaveBeenCalledWith("prowlarr", {
      url: "http://new:9696",
      api_key: "",
      categories: [11, 22],
      rss_poll_interval_minutes: 60,
    });
  });

  it("drops invalid category entries", () => {
    const { handleUpdateSetting } = renderCard();

    fireEvent.change(screen.getByPlaceholderText("7030, 7020"), {
      target: { value: "1, foo, 2" },
    });
    fireEvent.blur(screen.getByPlaceholderText("7030, 7020"));

    expect(handleUpdateSetting).toHaveBeenCalledWith(
      "prowlarr",
      expect.objectContaining({ categories: [1, 2] })
    );
  });

  it("falls back to a 30 minute interval when the value is invalid", () => {
    const { handleUpdateSetting } = renderCard();

    fireEvent.change(screen.getByPlaceholderText("settings.prowlarrUrlPlaceholder"), {
      target: { value: "http://p" },
    });
    fireEvent.blur(screen.getByPlaceholderText("settings.prowlarrUrlPlaceholder"));

    expect(handleUpdateSetting).toHaveBeenCalledWith(
      "prowlarr",
      expect.objectContaining({ rss_poll_interval_minutes: 30 })
    );
  });

  it("saves when the RSS interval changes", () => {
    const { handleUpdateSetting } = renderCard();

    fireEvent.change(screen.getByRole("combobox"), { target: { value: "1440" } });

    expect(handleUpdateSetting).toHaveBeenCalledWith(
      "prowlarr",
      expect.objectContaining({ rss_poll_interval_minutes: 1440 })
    );
  });

  it("disables the test button until a URL and API key are set", () => {
    const { handleUpdateSetting } = renderCard();
    const button = screen.getByRole("button", { name: "settings.testConnection" });
    expect(button).toBeDisabled();

    fireEvent.change(screen.getByPlaceholderText("settings.prowlarrUrlPlaceholder"), {
      target: { value: "http://p" },
    });
    fireEvent.change(screen.getByPlaceholderText("settings.prowlarrApiKeyPlaceholder"), {
      target: { value: "k" },
    });
    expect(button).toBeEnabled();
    expect(handleUpdateSetting).not.toHaveBeenCalled();
  });
});
