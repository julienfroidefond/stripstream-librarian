import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { MetadataProvidersCard } from "@/app/(app)/settings/components/MetadataProvidersCard";

const fetchMock = vi.fn();

const providers = [
  { id: "google_books", label: "Google Books", requires_api_key: true },
  { id: "comicvine", label: "Comicvine", requires_api_key: true },
  { id: "openlibrary", label: "OpenLibrary", requires_api_key: false },
  { id: "anilist", label: "AniList", requires_api_key: false },
];

beforeEach(() => {
  fetchMock.mockReset();
  vi.stubGlobal("fetch", fetchMock);
});

describe("MetadataProvidersCard", () => {
  it("loads providers and saves the default provider", async () => {
    fetchMock.mockResolvedValue({ ok: true, json: async () => providers });
    const handleUpdateSetting = vi.fn().mockResolvedValue(undefined);
    render(<MetadataProvidersCard initialData={null} handleUpdateSetting={handleUpdateSetting} />);
    await waitFor(() => expect(screen.getByRole("button", { name: "OpenLibrary" })).toBeInTheDocument());
    await userEvent.click(screen.getByRole("button", { name: "Comicvine" }));
    expect(handleUpdateSetting).toHaveBeenCalledWith(
      "metadata_providers",
      expect.objectContaining({ default_provider: "comicvine", metadata_language: "en" }),
    );
  });

  it("saves the metadata language", async () => {
    fetchMock.mockResolvedValue({ ok: true, json: async () => providers });
    const handleUpdateSetting = vi.fn().mockResolvedValue(undefined);
    render(<MetadataProvidersCard initialData={null} handleUpdateSetting={handleUpdateSetting} />);
    await waitFor(() => expect(screen.getByRole("button", { name: "OpenLibrary" })).toBeInTheDocument());
    await userEvent.click(screen.getByText("Français"));
    expect(handleUpdateSetting).toHaveBeenCalledWith(
      "metadata_providers",
      expect.objectContaining({ metadata_language: "fr" }),
    );
  });

  it("prefills api keys and saves them on blur", async () => {
    fetchMock.mockResolvedValue({ ok: true, json: async () => providers });
    const handleUpdateSetting = vi.fn().mockResolvedValue(undefined);
    render(
      <MetadataProvidersCard
        initialData={{ default_provider: "comicvine", metadata_language: "fr", comicvine: { api_key: "ck" }, google_books: { api_key: "gk" } }}
        handleUpdateSetting={handleUpdateSetting}
      />,
    );
    const google = screen.getByDisplayValue("gk");
    await userEvent.clear(google);
    await userEvent.type(google, "new-key");
    await userEvent.tab();
    expect(handleUpdateSetting).toHaveBeenCalledWith(
      "metadata_providers",
      expect.objectContaining({ default_provider: "comicvine", metadata_language: "fr", google_books: { api_key: "new-key" } }),
    );
  });

  it("saves the comicvine key on blur", async () => {
    fetchMock.mockResolvedValue({ ok: true, json: async () => providers });
    const handleUpdateSetting = vi.fn().mockResolvedValue(undefined);
    render(<MetadataProvidersCard initialData={{ default_provider: "comicvine" }} handleUpdateSetting={handleUpdateSetting} />);
    const comicvine = screen.getByPlaceholderText("settings.comicvinePlaceholder");
    await userEvent.type(comicvine, "cv-key");
    await userEvent.tab();
    expect(handleUpdateSetting).toHaveBeenCalledWith(
      "metadata_providers",
      expect.objectContaining({ comicvine: { api_key: "cv-key" } }),
    );
  });

  it("omits fields with empty values and tolerates a failing provider list", async () => {
    fetchMock.mockResolvedValue({ ok: false, json: async () => ({}) });
    const handleUpdateSetting = vi.fn().mockResolvedValue(undefined);
    render(<MetadataProvidersCard initialData={{ default_provider: "comicvine" }} handleUpdateSetting={handleUpdateSetting} />);
    await waitFor(() => expect(fetchMock).toHaveBeenCalledWith("/api/metadata/providers"));
    await userEvent.click(screen.getByText("English"));
    expect(handleUpdateSetting).toHaveBeenCalledWith("metadata_providers", {
      default_provider: "comicvine",
      metadata_language: "en",
    });
  });

  it("handles a rejected provider list", async () => {
    fetchMock.mockRejectedValue(new Error("network"));
    render(<MetadataProvidersCard initialData={{ default_provider: "google_books" }} handleUpdateSetting={vi.fn()} />);
    await waitFor(() => expect(screen.getByText("settings.metadataProviders")).toBeInTheDocument());
  });
});
