"use client";

import { useState } from "react";
import { Card, CardHeader, CardTitle, CardDescription, CardContent, Button, FormField, FormInput, Icon } from "@/app/components/ui";
import { useTranslation } from "@/lib/i18n/context";

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
  const [isTesting, setIsTesting] = useState(false);
  const [testResult, setTestResult] = useState<{ success: boolean; message: string } | null>(null);
  const [showHelp, setShowHelp] = useState(false);

  function saveTelegram(token?: string, chat?: string, en?: boolean, ev?: typeof events) {
    handleUpdateSetting("telegram", {
      bot_token: token ?? botToken,
      chat_id: chat ?? chatId,
      enabled: en ?? enabled,
      events: ev ?? events,
    });
  }

  async function handleTestConnection() {
    setIsTesting(true);
    setTestResult(null);
    try {
      const resp = await fetch("/api/telegram/test");
      const data = await resp.json();
      if (data.error) {
        setTestResult({ success: false, message: data.error });
      } else {
        setTestResult(data);
      }
    } catch {
      setTestResult({ success: false, message: "Failed to connect" });
    } finally {
      setIsTesting(false);
    }
  }

  return (
    <Card className="mb-6">
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          <Icon name="bell" size="md" />
          {t("settings.telegram")}
        </CardTitle>
        <CardDescription>{t("settings.telegramDesc")}</CardDescription>
      </CardHeader>
      <CardContent>
        <div className="space-y-4">
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
                  <p className="text-muted-foreground" dangerouslySetInnerHTML={{ __html: t("settings.telegramHelpBot") }} />
                </div>
                <div>
                  <p className="font-medium mb-1">2. Chat ID</p>
                  <p className="text-muted-foreground" dangerouslySetInnerHTML={{ __html: t("settings.telegramHelpChat") }} />
                </div>
                <div>
                  <p className="font-medium mb-1">3. Group chat</p>
                  <p className="text-muted-foreground" dangerouslySetInnerHTML={{ __html: t("settings.telegramHelpGroup") }} />
                </div>
              </div>
            )}
          </div>

          <div className="flex items-center gap-3">
            <label className="relative inline-flex items-center cursor-pointer">
              <input
                type="checkbox"
                checked={enabled}
                onChange={(e) => {
                  setEnabled(e.target.checked);
                  saveTelegram(undefined, undefined, e.target.checked);
                }}
                className="sr-only peer"
              />
              <div className="w-11 h-6 bg-muted rounded-full peer peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:border-gray-300 after:border after:rounded-full after:h-5 after:w-5 after:transition-all peer-checked:bg-primary"></div>
            </label>
            <span className="text-sm font-medium text-foreground">{t("settings.telegramEnabled")}</span>
          </div>

          <div className="flex gap-4">
            <FormField className="flex-1">
              <label className="text-sm font-medium text-muted-foreground mb-1 block">{t("settings.botToken")}</label>
              <FormInput
                type="password" autoComplete="off"
                placeholder={t("settings.botTokenPlaceholder")}
                value={botToken}
                onChange={(e) => setBotToken(e.target.value)}
                onBlur={() => saveTelegram()}
              />
            </FormField>
          </div>
          <div className="flex gap-4">
            <FormField className="flex-1">
              <label className="text-sm font-medium text-muted-foreground mb-1 block">{t("settings.chatId")}</label>
              <FormInput
                type="text"
                placeholder={t("settings.chatIdPlaceholder")}
                value={chatId}
                onChange={(e) => setChatId(e.target.value)}
                onBlur={() => saveTelegram()}
              />
            </FormField>
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
                      <label key={key} className="flex items-center justify-between py-1.5 cursor-pointer group">
                        <span className="text-sm text-foreground group-hover:text-foreground/80">{label}</span>
                        <div className="relative">
                          <input
                            type="checkbox"
                            checked={events[key]}
                            onChange={(e) => {
                              const updated = { ...events, [key]: e.target.checked };
                              setEvents(updated);
                              saveTelegram(undefined, undefined, undefined, updated);
                            }}
                            className="sr-only peer"
                          />
                          <div className="w-9 h-5 bg-muted rounded-full peer peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:border-gray-300 after:border after:rounded-full after:h-4 after:w-4 after:transition-all peer-checked:bg-primary" />
                        </div>
                      </label>
                    ))}
                  </div>
                </div>
              ))}
            </div>
          </div>

          <div className="flex items-center gap-3">
            <Button
              onClick={handleTestConnection}
              disabled={isTesting || !botToken || !chatId || !enabled}
            >
              {isTesting ? (
                <>
                  <Icon name="spinner" size="sm" className="animate-spin -ml-1 mr-2" />
                  {t("settings.testing")}
                </>
              ) : (
                <>
                  <Icon name="refresh" size="sm" className="mr-2" />
                  {t("settings.testConnection")}
                </>
              )}
            </Button>
            {testResult && (
              <span className={`text-sm font-medium ${testResult.success ? "text-success" : "text-destructive"}`}>
                {testResult.message}
              </span>
            )}
          </div>
        </div>
      </CardContent>
    </Card>
  );
}
