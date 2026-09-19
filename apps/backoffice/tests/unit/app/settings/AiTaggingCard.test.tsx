import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { AiTaggingCard } from "@/app/(app)/settings/components/AiTaggingCard";

const fetchMock = vi.fn();

beforeEach(() => {
  fetchMock.mockReset();
  vi.stubGlobal("fetch", fetchMock);
});

describe("AiTaggingCard", () => {
  it("renders defaults and saves the enabled flag", async () => {
    const handleUpdateSetting = vi.fn().mockResolvedValue(undefined);
    render(<AiTaggingCard initialData={null} handleUpdateSetting={handleUpdateSetting} />);
    expect(screen.getByRole("checkbox")).not.toBeChecked();
    await userEvent.click(screen.getByRole("checkbox"));
    expect(handleUpdateSetting).toHaveBeenCalledWith(
      "ai_tagging",
      expect.objectContaining({ enabled: true, model: "openrouter/free", max_tags: 5 }),
    );
  });

  it("uses provided initial data", () => {
    render(
      <AiTaggingCard
        initialData={{ enabled: true, base_url: "http://x", api_key: "k", model: "m", max_tags: 9, prompt: "p" }}
        handleUpdateSetting={vi.fn()}
      />,
    );
    expect(screen.getByDisplayValue("http://x")).toBeInTheDocument();
    expect(screen.getByDisplayValue("m")).toBeInTheDocument();
    expect(screen.getByDisplayValue("9")).toBeInTheDocument();
    expect(screen.getByDisplayValue("p")).toBeInTheDocument();
  });

  it("saves edited fields on blur", async () => {
    const handleUpdateSetting = vi.fn().mockResolvedValue(undefined);
    render(<AiTaggingCard initialData={null} handleUpdateSetting={handleUpdateSetting} />);
    const url = screen.getByDisplayValue("https://openrouter.ai/api/v1");
    await userEvent.clear(url);
    await userEvent.type(url, "http://localhost:1234");
    await userEvent.tab();
    expect(handleUpdateSetting).toHaveBeenCalledWith(
      "ai_tagging",
      expect.objectContaining({ base_url: "http://localhost:1234" }),
    );
  });

  it("clamps max_tags between 1 and 10", async () => {
    const handleUpdateSetting = vi.fn().mockResolvedValue(undefined);
    render(<AiTaggingCard initialData={null} handleUpdateSetting={handleUpdateSetting} />);
    const max = screen.getByDisplayValue("5");

    fireEvent.change(max, { target: { value: "99" } });
    fireEvent.blur(max);
    expect(handleUpdateSetting).toHaveBeenLastCalledWith("ai_tagging", expect.objectContaining({ max_tags: 10 }));

    fireEvent.change(max, { target: { value: "-5" } });
    fireEvent.blur(max);
    expect(handleUpdateSetting).toHaveBeenLastCalledWith("ai_tagging", expect.objectContaining({ max_tags: 1 }));

    fireEvent.change(max, { target: { value: "" } });
    fireEvent.blur(max);
    expect(handleUpdateSetting).toHaveBeenLastCalledWith("ai_tagging", expect.objectContaining({ max_tags: 5 }));
  });

  it("saves the api key, model and prompt on blur", async () => {
    const handleUpdateSetting = vi.fn().mockResolvedValue(undefined);
    render(<AiTaggingCard initialData={null} handleUpdateSetting={handleUpdateSetting} />);

    const key = screen.getByPlaceholderText("sk-or-v1-...");
    fireEvent.change(key, { target: { value: "sk-1" } });
    fireEvent.blur(key);
    expect(handleUpdateSetting).toHaveBeenLastCalledWith("ai_tagging", expect.objectContaining({ api_key: "sk-1" }));

    const model = screen.getByDisplayValue("openrouter/free");
    fireEvent.change(model, { target: { value: "openrouter/auto" } });
    fireEvent.blur(model);
    expect(handleUpdateSetting).toHaveBeenLastCalledWith("ai_tagging", expect.objectContaining({ model: "openrouter/auto" }));

    const prompt = document.querySelector("textarea") as HTMLTextAreaElement;
    fireEvent.change(prompt, { target: { value: "custom prompt" } });
    fireEvent.blur(prompt);
    expect(handleUpdateSetting).toHaveBeenLastCalledWith("ai_tagging", expect.objectContaining({ prompt: "custom prompt" }));
  });

  it("restores the default prompt", async () => {
    const handleUpdateSetting = vi.fn().mockResolvedValue(undefined);
    render(<AiTaggingCard initialData={{ prompt: "custom" }} handleUpdateSetting={handleUpdateSetting} />);
    const restore = screen.getByRole("button", { name: "settings.aiRestoreDefaultPrompt" });
    await userEvent.click(restore);
    expect(handleUpdateSetting).toHaveBeenCalledWith(
      "ai_tagging",
      expect.objectContaining({ prompt: expect.stringContaining("Suggest up to") }),
    );
  });

  it("tests the connection successfully", async () => {
    fetchMock.mockResolvedValue({ ok: true, json: async () => ({ message: "OK" }) });
    render(<AiTaggingCard initialData={null} handleUpdateSetting={vi.fn()} />);
    await userEvent.click(screen.getByRole("button", { name: "settings.aiTest" }));
    await waitFor(() => expect(screen.getByText("OK")).toBeInTheDocument());
  });

  it("reports an error returned by the API", async () => {
    fetchMock.mockResolvedValue({ ok: false, json: async () => ({ error: "bad key" }) });
    render(<AiTaggingCard initialData={null} handleUpdateSetting={vi.fn()} />);
    await userEvent.click(screen.getByRole("button", { name: "settings.aiTest" }));
    await waitFor(() => expect(screen.getByText("bad key")).toBeInTheDocument());
  });

  it("falls back to the translated error on failure", async () => {
    fetchMock.mockRejectedValue(new Error("network"));
    render(<AiTaggingCard initialData={null} handleUpdateSetting={vi.fn()} />);
    await userEvent.click(screen.getByRole("button", { name: "settings.aiTest" }));
    await waitFor(() => expect(screen.getByText("settings.aiTestError")).toBeInTheDocument());
  });

  it("falls back to the translated error when the payload is empty", async () => {
    fetchMock.mockResolvedValue({ ok: false, json: async () => ({}) });
    render(<AiTaggingCard initialData={null} handleUpdateSetting={vi.fn()} />);
    await userEvent.click(screen.getByRole("button", { name: "settings.aiTest" }));
    await waitFor(() => expect(screen.getByText("settings.aiTestError")).toBeInTheDocument());
  });
});
