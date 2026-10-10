"use client";

import { useState } from "react";
import { SettingsCard, SettingsField, FormInput, Icon, Switch } from "@/app/components/ui";
import { SafeHtml } from "@/app/components/SafeHtml";
import { useTranslation } from "@/lib/i18n/context";
import { TestConnectionButton } from "./TestConnectionButton";

export const DEFAULT_EVENTS = {
  scan_completed: true,
  scan_failed: true,
  scan_cancelled: true,
  thumbnail_completed: true,
  thumbnail_failed: true,
  conversion_completed: true,
  conversion_failed: true,
  metadata_approved: true,
  metadata_batch_completed: true,
  metadata_batch_failed: true,
  metadata_refresh_completed: true,
  metadata_refresh_failed: true,
  reading_status_match_completed: true,
  reading_status_match_failed: true,
  reading_status_push_completed: true,
  reading_status_push_failed: true,
  download_detection_completed: true,
  download_detection_failed: true,
};

export function TelegramCard({ handleUpdateSetting, initialData }: { handleUpdateSetting: (key: string, value: unknown) => Promise<void>; initialData: Record<string, unknown> | null }) {
  const { t } = useTranslation();
  const [botToken, setBotToken] = useState(initialData?.bot_token ? String(initialData.bot_token) : "");
  const [chatId, setChatId] = useState(initialData?.chat_id ? String(initialData.chat_id) : "");
  const [enabled, setEnabled] = useState(initialData?.enabled === true);
  const [events, setEvents] = useState(
    initialData?.events ? { ...DEFAULT_EVENTS, ...(initialData.events as Record<string, boolean>) } : DEFAULT_EVENTS
  );
  const [showHelp, setShowHelp] = useState(false);

  function saveTelegram(token?: string, chat?: string, en?: boolean, ev?: typeof events) {
    handleUpdateSetting("telegram", {
      bot_token: token ?? botToken,
      chat_id: chat ?? chatId,
      enabled: en ?? enabled,
      events: ev ?? events,
    });
  }

  return (
    <SettingsCard icon="bell" title={t("settings.telegram")} description={t("settings.telegramDesc")}>
      {/* Setup guide */}
      <div>
        <button
          type="button"
          onClick={() => setShowHelp(!showHelp)}
          className="text-sm text-primary hover:text-primary/80 flex items-center gap-1 transition-colors"
        >
          <Icon name={showHelp ? "chevronDown" : "chevronRight"} size="sm" />
          {t("settings.telegramHelp")}
        </button>
        {showHelp && (
          <div className="mt-3 p-4 rounded-lg bg-muted/30 space-y-3 text-sm text-foreground">
            <div>
              <p className="font-medium mb-1">1. Bot Token</p>
              <SafeHtml html={t("settings.telegramHelpBot")} className="text-muted-foreground" as="p" />
            </div>
            <div>
              <p className="font-medium mb-1">2. Chat ID</p>
              <SafeHtml html={t("settings.telegramHelpChat")} className="text-muted-foreground" as="p" />
            </div>
            <div>
              <p className="font-medium mb-1">3. Group chat</p>
              <SafeHtml html={t("settings.telegramHelpGroup")} className="text-muted-foreground" as="p" />
            </div>
          </div>
        )}
      </div>

      <div className="flex items-center gap-3">
        <Switch
          checked={enabled}
          onChange={(e) => {
            setEnabled(e.target.checked);
            saveTelegram(undefined, undefined, e.target.checked);
          }}
        />
        <span className="text-sm font-medium text-foreground">{t("settings.telegramEnabled")}</span>
      </div>

      <div className="flex gap-4">
        <SettingsField className="flex-1" label={t("settings.botToken")}>
          <FormInput
            type="password" autoComplete="off"
            placeholder={t("settings.botTokenPlaceholder")}
            value={botToken}
            onChange={(e) => setBotToken(e.target.value)}
            onBlur={() => saveTelegram()}
          />
        </SettingsField>
      </div>
      <div className="flex gap-4">
        <SettingsField className="flex-1" label={t("settings.chatId")}>
          <FormInput
            type="text"
            placeholder={t("settings.chatIdPlaceholder")}
            value={chatId}
            onChange={(e) => setChatId(e.target.value)}
            onBlur={() => saveTelegram()}
          />
        </SettingsField>
      </div>

      {/* Event toggles grouped by category */}
      <div className="border-t border-border/50 pt-4">
        <h4 className="text-sm font-medium text-foreground mb-4">{t("settings.telegramEvents")}</h4>
        <div className="grid grid-cols-2 gap-x-6 gap-y-5">
          {([
            {
              category: t("settings.eventCategoryScan"),
              icon: "search" as const,
              items: [
                { key: "scan_completed" as const, label: t("settings.eventCompleted") },
                { key: "scan_failed" as const, label: t("settings.eventFailed") },
                { key: "scan_cancelled" as const, label: t("settings.eventCancelled") },
              ],
            },
            {
              category: t("settings.eventCategoryThumbnail"),
              icon: "image" as const,
              items: [
                { key: "thumbnail_completed" as const, label: t("settings.eventCompleted") },
                { key: "thumbnail_failed" as const, label: t("settings.eventFailed") },
              ],
            },
            {
              category: t("settings.eventCategoryConversion"),
              icon: "refresh" as const,
              items: [
                { key: "conversion_completed" as const, label: t("settings.eventCompleted") },
                { key: "conversion_failed" as const, label: t("settings.eventFailed") },
              ],
            },
            {
              category: t("settings.eventCategoryMetadata"),
              icon: "tag" as const,
              items: [
                { key: "metadata_approved" as const, label: t("settings.eventLinked") },
                { key: "metadata_batch_completed" as const, label: t("settings.eventBatchCompleted") },
                { key: "metadata_batch_failed" as const, label: t("settings.eventBatchFailed") },
                { key: "metadata_refresh_completed" as const, label: t("settings.eventRefreshCompleted") },
                { key: "metadata_refresh_failed" as const, label: t("settings.eventRefreshFailed") },
              ],
            },
            {
              category: t("settings.eventCategoryReadingStatus"),
              icon: "books" as const,
              items: [
                { key: "reading_status_match_completed" as const, label: t("settings.eventMatchCompleted") },
                { key: "reading_status_match_failed" as const, label: t("settings.eventMatchFailed") },
                { key: "reading_status_push_completed" as const, label: t("settings.eventPushCompleted") },
                { key: "reading_status_push_failed" as const, label: t("settings.eventPushFailed") },
              ],
            },
            {
              category: t("settings.eventCategoryDownloadDetection"),
              icon: "download" as const,
              items: [
                { key: "download_detection_completed" as const, label: t("settings.eventCompleted") },
                { key: "download_detection_failed" as const, label: t("settings.eventFailed") },
              ],
            },
          ]).map(({ category, icon, items }) => (
            <div key={category}>
              <p className="text-xs font-medium text-muted-foreground uppercase tracking-wide mb-2 flex items-center gap-1.5">
                <Icon name={icon} size="sm" className="text-muted-foreground" />
                {category}
              </p>
              <div className="space-y-1">
                {items.map(({ key, label }) => (
                  <Switch
                    key={key}
                    size="sm"
                    labelPosition="left"
                    className="py-1.5 group"
                    label={<span className="text-sm text-foreground group-hover:text-foreground/80">{label}</span>}
                    checked={events[key]}
                    onChange={(e) => {
                      const updated = { ...events, [key]: e.target.checked };
                      setEvents(updated);
                      saveTelegram(undefined, undefined, undefined, updated);
                    }}
                  />
                ))}
              </div>
            </div>
          ))}
        </div>
      </div>

      <div className="flex items-center gap-3">
        <TestConnectionButton
          endpoint="/api/telegram/test"
          disabled={!botToken || !chatId || !enabled}
        />
      </div>
    </SettingsCard>
  );
}
