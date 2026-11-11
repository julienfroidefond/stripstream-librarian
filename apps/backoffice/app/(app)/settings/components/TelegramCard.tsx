"use client";

import { useState } from "react";
import { SettingsCard, SettingsRow, FormInput, Icon, Switch } from "@/app/components/ui";
import { SafeHtml } from "@/app/components/SafeHtml";
import { useTranslation } from "@/lib/i18n/context";
import {
  DEFAULT_TELEGRAM_EVENTS,
  type TelegramSettings,
} from "@/lib/settingsSections";
import { useSettingsSection } from "../SettingsDraftProvider";
import { TestConnectionButton } from "./TestConnectionButton";

export const DEFAULT_EVENTS = DEFAULT_TELEGRAM_EVENTS;

export function TelegramCard() {
  const { t } = useTranslation();
  const section = useSettingsSection<TelegramSettings>("telegram");
  const value = section.value;
  const events = value.events ?? DEFAULT_TELEGRAM_EVENTS;
  const [showHelp, setShowHelp] = useState(false);

  return (
    <div className="grid grid-cols-1 gap-5 xl:grid-cols-2 xl:items-start">
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
            <div className="settings-well mt-3 space-y-3 rounded-xl p-4 text-sm text-foreground">
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

        <SettingsRow label={t("settings.telegramEnabled")} controlClassName="sm:max-w-none">
          <Switch
            checked={value.enabled}
            onChange={(e) => {
              section.patch({ enabled: e.target.checked });
              void section.save();
            }}
          />
        </SettingsRow>

        <SettingsRow label={t("settings.botToken")} htmlFor="telegram-bot-token">
          <FormInput
            id="telegram-bot-token"
            type="password" autoComplete="off"
            placeholder={t("settings.botTokenPlaceholder")}
            value={value.bot_token}
            onChange={(e) => section.patch({ bot_token: e.target.value })}
            onBlur={() => void section.save()}
          />
        </SettingsRow>
        <SettingsRow label={t("settings.chatId")} htmlFor="telegram-chat-id">
          <FormInput
            id="telegram-chat-id"
            type="text"
            placeholder={t("settings.chatIdPlaceholder")}
            value={value.chat_id}
            onChange={(e) => section.patch({ chat_id: e.target.value })}
            onBlur={() => void section.save()}
          />
        </SettingsRow>

        <div className="flex items-center gap-3 pt-1">
          <TestConnectionButton
            endpoint="/api/telegram/test"
            disabled={!value.bot_token || !value.chat_id || !value.enabled}
          />
        </div>
      </SettingsCard>

      <SettingsCard icon="bell" title={t("settings.telegramEvents")}>
          <div className="grid grid-cols-2 gap-x-8 gap-y-5">
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
                  { key: "download_detection_partial" as const, label: t("settings.eventPartial") },
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
                        section.patch({ events: { ...events, [key]: e.target.checked } });
                        void section.save();
                      }}
                    />
                  ))}
                </div>
              </div>
            ))}
          </div>
      </SettingsCard>
    </div>
  );
}
