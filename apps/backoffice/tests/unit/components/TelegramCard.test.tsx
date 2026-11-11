import { fireEvent, screen, waitFor } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { TelegramCard } from "@/app/(app)/settings/components/TelegramCard";
import { renderWithSettingsDraft } from "../helpers/settingsDraft";

function renderCard(telegram?: Record<string, unknown>) {
  return renderWithSettingsDraft(<TelegramCard />, { initial: { telegram } });
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

  it("toggles the notification switch and saves", async () => {
    const { writer } = renderCard({ bot_token: "123:abc", chat_id: "-100", enabled: false });

    fireEvent.click(screen.getAllByRole("checkbox")[0]);

    await waitFor(() => expect(writer).toHaveBeenCalledWith(
      "telegram",
      expect.objectContaining({ enabled: true, bot_token: "123:abc", chat_id: "-100" }),
    ));
  });

  it("saves the bot token on blur", async () => {
    const { writer } = renderCard();

    fireEvent.change(screen.getByPlaceholderText("settings.botTokenPlaceholder"), {
      target: { value: "123:abc" },
    });
    fireEvent.blur(screen.getByPlaceholderText("settings.botTokenPlaceholder"));

    await waitFor(() => expect(writer).toHaveBeenCalledWith(
      "telegram",
      expect.objectContaining({ bot_token: "123:abc" }),
    ));
  });

  it("rejects enabling Telegram without a token and chat id", async () => {
    const { writer } = renderCard();

    fireEvent.click(screen.getAllByRole("checkbox")[0]);
    // Wait for the rejected optimistic edit to roll back, then assert no write.
    await waitFor(() => {
      expect((screen.getAllByRole("checkbox")[0] as HTMLInputElement).checked).toBe(false);
    });

    expect(writer).not.toHaveBeenCalled();
  });

  it("saves the chat id on blur", async () => {
    const { writer } = renderCard();

    const chat = screen.getByPlaceholderText("settings.chatIdPlaceholder");
    fireEvent.change(chat, { target: { value: "-100" } });
    fireEvent.blur(chat);

    await waitFor(() => expect(writer).toHaveBeenCalledWith(
      "telegram",
      expect.objectContaining({ chat_id: "-100" }),
    ));
  });

  it("updates a single event flag and saves the whole event map", async () => {
    const { writer, container } = renderCard();
    const boxes = checkboxes(container);
    // 1 master switch + 19 event toggles, including the dedicated partial event.
    expect(boxes).toHaveLength(20);
    expect(screen.getByText("settings.eventPartial")).toBeInTheDocument();

    fireEvent.click(boxes[1]);

    await waitFor(() => expect(writer).toHaveBeenCalledWith(
      "telegram",
      expect.objectContaining({
        events: expect.objectContaining({ scan_completed: false, scan_failed: true }),
      }),
    ));
  });

  it("toggles the setup help panel", () => {
    renderCard();

    expect(screen.queryByText("settings.telegramHelpBot", { exact: false })).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "settings.telegramHelp" }));

    expect(screen.getByText("1. Bot Token")).toBeInTheDocument();
    expect(screen.getByText("2. Chat ID")).toBeInTheDocument();
    expect(screen.getByText("3. Group chat")).toBeInTheDocument();
  });

  it("disables the test button until the bot is enabled and configured", async () => {
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
    await waitFor(() => expect(button).toBeEnabled());
  });
});
