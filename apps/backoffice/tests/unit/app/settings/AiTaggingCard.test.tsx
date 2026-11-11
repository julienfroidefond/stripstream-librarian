import { fireEvent, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { AiTaggingCard } from "@/app/(app)/settings/components/AiTaggingCard";
import { renderWithSettingsDraft } from "../../helpers/settingsDraft";

const fetchMock = vi.fn();

beforeEach(() => {
  fetchMock.mockReset();
  vi.stubGlobal("fetch", fetchMock);
});

function renderCard(initial?: Record<string, unknown>) {
  return renderWithSettingsDraft(<AiTaggingCard />, { initial });
}

describe("AiTaggingCard", () => {
  it("renders defaults and saves the enabled flag", async () => {
    const { writer } = renderCard();
    const genreToggle = screen.getByRole("checkbox", { name: "settings.aiGenreTagging" });
    const listsToggle = screen.getByRole("checkbox", { name: "settings.aiReadingListsEnabled" });
    expect(genreToggle).not.toBeChecked();
    expect(listsToggle).not.toBeChecked();
    await userEvent.click(genreToggle);

    await waitFor(() => expect(writer).toHaveBeenCalledWith(
      "ai_tagging",
      expect.objectContaining({ enabled: true, model: "openrouter/free", max_tags: 5 }),
    ));
  });

  it("saves the reading list flag independently", async () => {
    const { writer } = renderCard();
    await userEvent.click(screen.getByRole("checkbox", { name: "settings.aiReadingListsEnabled" }));

    await waitFor(() => expect(writer).toHaveBeenCalledWith(
      "ai_tagging",
      expect.objectContaining({ reading_lists_enabled: true, enabled: false }),
    ));
  });

  it("uses provided initial data", () => {
    renderCard({
      ai_tagging: {
        enabled: true,
        base_url: "http://x",
        api_key: "k",
        model: "m",
        max_tags: 9,
        prompt: "p",
        reading_list_prompt: "lists",
      },
    });
    expect(screen.getByDisplayValue("http://x")).toBeInTheDocument();
    expect(screen.getByDisplayValue("m")).toBeInTheDocument();
    expect(screen.getByDisplayValue("9")).toBeInTheDocument();
    expect(screen.getByDisplayValue("p")).toBeInTheDocument();
    expect(screen.getByDisplayValue("lists")).toBeInTheDocument();
  });

  it("saves edited fields on blur", async () => {
    const { writer } = renderCard();
    const url = screen.getByDisplayValue("https://openrouter.ai/api/v1");
    await userEvent.clear(url);
    await userEvent.type(url, "http://localhost:1234");
    await userEvent.tab();

    await waitFor(() => expect(writer).toHaveBeenCalledWith(
      "ai_tagging",
      expect.objectContaining({ base_url: "http://localhost:1234" }),
    ));
  });

  it("clamps max_tags between 1 and 10", async () => {
    const { writer } = renderCard();
    const max = screen.getByDisplayValue("5");

    fireEvent.change(max, { target: { value: "99" } });
    fireEvent.blur(max);
    await waitFor(() => expect(writer).toHaveBeenLastCalledWith(
      "ai_tagging",
      expect.objectContaining({ max_tags: 10 }),
    ));

    fireEvent.change(max, { target: { value: "-5" } });
    fireEvent.blur(max);
    await waitFor(() => expect(writer).toHaveBeenLastCalledWith(
      "ai_tagging",
      expect.objectContaining({ max_tags: 1 }),
    ));

    fireEvent.change(max, { target: { value: "" } });
    fireEvent.blur(max);
    await waitFor(() => expect(writer).toHaveBeenLastCalledWith(
      "ai_tagging",
      expect.objectContaining({ max_tags: 5 }),
    ));
  });

  it("saves the api key, model and prompt on blur", async () => {
    const { writer } = renderCard();

    const key = screen.getByPlaceholderText("sk-or-v1-...");
    fireEvent.change(key, { target: { value: "sk-1" } });
    fireEvent.blur(key);
    await waitFor(() => expect(writer).toHaveBeenLastCalledWith(
      "ai_tagging",
      expect.objectContaining({ api_key: "sk-1" }),
    ));

    const model = screen.getByDisplayValue("openrouter/free");
    fireEvent.change(model, { target: { value: "openrouter/auto" } });
    fireEvent.blur(model);
    await waitFor(() => expect(writer).toHaveBeenLastCalledWith(
      "ai_tagging",
      expect.objectContaining({ model: "openrouter/auto" }),
    ));

    const prompt = document.querySelector("textarea") as HTMLTextAreaElement;
    fireEvent.change(prompt, { target: { value: "custom prompt" } });
    fireEvent.blur(prompt);
    await waitFor(() => expect(writer).toHaveBeenLastCalledWith(
      "ai_tagging",
      expect.objectContaining({ prompt: "custom prompt" }),
    ));
  });

  it("restores the default prompt", async () => {
    const { writer } = renderCard({ ai_tagging: { prompt: "custom" } });
    const restore = screen.getByRole("button", { name: "settings.aiRestoreDefaultPrompt" });
    await userEvent.click(restore);

    await waitFor(() => expect(writer).toHaveBeenCalledWith(
      "ai_tagging",
      expect.objectContaining({ prompt: expect.stringContaining("Suggest up to") }),
    ));
  });

  it("saves and restores the reading list prompt", async () => {
    const { writer } = renderCard({ ai_tagging: { reading_list_prompt: "custom lists" } });
    const prompt = screen.getByDisplayValue("custom lists");
    fireEvent.change(prompt, { target: { value: "new lists prompt" } });
    fireEvent.blur(prompt);
    await waitFor(() => expect(writer).toHaveBeenLastCalledWith(
      "ai_tagging",
      expect.objectContaining({ reading_list_prompt: "new lists prompt" }),
    ));

    await userEvent.click(
      screen.getByRole("button", { name: "settings.aiRestoreDefaultListsPrompt" })
    );
    await waitFor(() => expect(writer).toHaveBeenLastCalledWith(
      "ai_tagging",
      expect.objectContaining({ reading_list_prompt: expect.stringContaining("{{series}}") }),
    ));
  });

  it("tests the connection successfully", async () => {
    fetchMock.mockResolvedValue({ ok: true, json: async () => ({ message: "OK" }) });
    renderCard();
    await userEvent.click(screen.getByRole("button", { name: "settings.aiTest" }));
    await waitFor(() => expect(screen.getByText("OK")).toBeInTheDocument());
  });

  it("reports an error returned by the API", async () => {
    fetchMock.mockResolvedValue({ ok: false, json: async () => ({ error: "bad key" }) });
    renderCard();
    await userEvent.click(screen.getByRole("button", { name: "settings.aiTest" }));
    await waitFor(() => expect(screen.getByText("bad key")).toBeInTheDocument());
  });

  it("falls back to the translated error on failure", async () => {
    fetchMock.mockRejectedValue(new Error("network"));
    renderCard();
    await userEvent.click(screen.getByRole("button", { name: "settings.aiTest" }));
    await waitFor(() => expect(screen.getByText("settings.aiTestError")).toBeInTheDocument());
  });

  it("falls back to the translated error when the payload is empty", async () => {
    fetchMock.mockResolvedValue({ ok: false, json: async () => ({}) });
    renderCard();
    await userEvent.click(screen.getByRole("button", { name: "settings.aiTest" }));
    await waitFor(() => expect(screen.getByText("settings.aiTestError")).toBeInTheDocument());
  });
});
