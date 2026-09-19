import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { TelegramCard } from "@/app/(app)/settings/components/TelegramCard";

function renderCard(initialData: Record<string, unknown> | null = null) {
  const handleUpdateSetting = vi.fn().mockResolvedValue(undefined);
  const { container } = render(
    <TelegramCard handleUpdateSetting={handleUpdateSetting} initialData={initialData} />
  );
  return { handleUpdateSetting, container };
}

function checkboxes(container: HTMLElement) {
  return Array.from(container.querySelectorAll<HTMLInputElement>('input[type="checkbox"]'));
}

describe("TelegramCard", () => {
  it("renders the title, event categories and empty defaults", () => {
    renderCard();

    expect(screen.getByText("settings.telegram")).toBeInTheDocument();
    expect(screen.getByText("settings.telegramEvents")).toBeInTheDocument();
    expect(screen.getByText("settings.eventCategoryScan")).toBeInTheDocument();
    expect(screen.getByText("settings.eventCategoryReadingStatus")).toBeInTheDocument();
    expect(screen.getByPlaceholderText("settings.botTokenPlaceholder")).toHaveValue("");
    expect(screen.getByPlaceholderText("settings.chatIdPlaceholder")).toHaveValue("");
  });

  it("prefills the token, chat id and merges stored events", () => {
    renderCard({
      bot_token: "123:abc",
      chat_id: "-100",
      enabled: true,
      events: { scan_completed: false },
    });

    expect(screen.getByPlaceholderText("settings.botTokenPlaceholder")).toHaveValue("123:abc");
    expect(screen.getByPlaceholderText("settings.chatIdPlaceholder")).toHaveValue("-100");
    const enabledToggle = screen.getAllByRole("checkbox")[0] as HTMLInputElement;
    expect(enabledToggle.checked).toBe(true);
  });

  it("toggles the notification switch and saves", () => {
    const { handleUpdateSetting } = renderCard();

    fireEvent.click(screen.getAllByRole("checkbox")[0]);

    expect(handleUpdateSetting).toHaveBeenCalledWith(
      "telegram",
      expect.objectContaining({ enabled: true, bot_token: "", chat_id: "" })
    );
  });

  it("saves the bot token on blur", () => {
    const { handleUpdateSetting } = renderCard();

    fireEvent.change(screen.getByPlaceholderText("settings.botTokenPlaceholder"), {
      target: { value: "123:abc" },
    });
    fireEvent.blur(screen.getByPlaceholderText("settings.botTokenPlaceholder"));

    expect(handleUpdateSetting).toHaveBeenCalledWith(
      "telegram",
      expect.objectContaining({ bot_token: "123:abc" })
    );
  });

  it("updates a single event flag and saves the whole event map", () => {
    const { handleUpdateSetting, container } = renderCard();
    const boxes = checkboxes(container);
    expect(boxes).toHaveLength(19);

    fireEvent.click(boxes[1]);

    expect(handleUpdateSetting).toHaveBeenCalledWith(
      "telegram",
      expect.objectContaining({
        events: expect.objectContaining({ scan_completed: false, scan_failed: true }),
      })
    );
  });

  it("toggles the setup help panel", () => {
    renderCard();

    expect(screen.queryByText("settings.telegramHelpBot", { exact: false })).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "settings.telegramHelp" }));

    expect(screen.getByText("1. Bot Token")).toBeInTheDocument();
    expect(screen.getByText("2. Chat ID")).toBeInTheDocument();
    expect(screen.getByText("3. Group chat")).toBeInTheDocument();
  });

  it("disables the test button until the bot is enabled and configured", () => {
    const { container } = renderCard();
    const button = screen.getByRole("button", { name: "settings.testConnection" });
    expect(button).toBeDisabled();

    fireEvent.change(screen.getByPlaceholderText("settings.botTokenPlaceholder"), {
      target: { value: "123:abc" },
    });
    fireEvent.change(screen.getByPlaceholderText("settings.chatIdPlaceholder"), {
      target: { value: "-100" },
    });
    expect(button).toBeDisabled();

    fireEvent.click(checkboxes(container)[0]);
    expect(button).toBeEnabled();
  });
});
