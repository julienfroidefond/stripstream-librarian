"use client";

import { useState, useEffect, useRef, useCallback } from "react";
import {
  Card, CardHeader, CardTitle, CardDescription, CardContent,
  Button, FormField, FormInput, FormSelect, Icon, toast,
} from "@/app/components/ui";
import { useTranslation } from "@/lib/i18n/context";
import type { LibraryDto, TelegramMonitorStatus, TelegramSourceDto } from "@/lib/api";

interface ChannelSuggestion {
  username: string | null;
  title: string;
  kind: string;
}

const SYNC_INTERVAL_VALUES = new Set(["30", "60", "1440", "43200"]);

async function tgFetch<T = unknown>(path: string, init?: RequestInit): Promise<T> {
  const res = await fetch(`/api/telegram-monitor/${path}`, {
    ...init,
    headers: { "Content-Type": "application/json", ...init?.headers },
  });
  if (!res.ok) {
    const text = await res.text();
    throw new Error(text || `Request failed (${res.status})`);
  }
  return res.json();
}


export function TelegramMonitorCard({ initialLibraries = [] }: { initialLibraries?: LibraryDto[] }) {
  const { t } = useTranslation();

  // Settings
  const [apiId, setApiId] = useState("");
  const [apiHash, setApiHash] = useState("");
  const [phone, setPhone] = useState("");
  const [syncInterval, setSyncInterval] = useState("30");
  const [savingSettings, setSavingSettings] = useState(false);

  // Auth
  const [status, setStatus] = useState<TelegramMonitorStatus | null>(null);
  const [codeSent, setCodeSent] = useState(false);
  const [code, setCode] = useState("");
  const [sendingCode, setSendingCode] = useState(false);
  const [verifying, setVerifying] = useState(false);
  const [disconnecting, setDisconnecting] = useState(false);

  // Sources
  const [sources, setSources] = useState<TelegramSourceDto[]>([]);
  const [newChannel, setNewChannel] = useState("");
  const [newChannelLibrary, setNewChannelLibrary] = useState("");
  const [addingSource, setAddingSource] = useState(false);
  const [libraries] = useState<LibraryDto[]>(initialLibraries);

  // Channel autocomplete
  const [suggestions, setSuggestions] = useState<ChannelSuggestion[]>([]);
  const [showSuggestions, setShowSuggestions] = useState(false);
  const [loadingSuggestions, setLoadingSuggestions] = useState(false);
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const autocompleteRef = useRef<HTMLDivElement>(null);

  // Sync result (shown after manual trigger from Jobs page or last scheduler run)
  const [syncResult] = useState<{ synced: number; new_books: number } | null>(null);
  const SYNC_INTERVAL_OPTIONS = [
    { value: "30", label: t("telegramMonitor.syncInterval30m") },
    { value: "60", label: t("telegramMonitor.syncInterval1h") },
    { value: "1440", label: t("telegramMonitor.syncInterval1d") },
    { value: "43200", label: t("telegramMonitor.syncInterval1mo") },
  ];

  useEffect(() => {
    loadAll();
  }, []);

  useEffect(() => {
    function handleClickOutside(e: MouseEvent) {
      if (autocompleteRef.current && !autocompleteRef.current.contains(e.target as Node)) {
        setShowSuggestions(false);
      }
    }
    document.addEventListener("mousedown", handleClickOutside);
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, []);

  const searchChannels = useCallback((q: string) => {
    if (debounceRef.current) clearTimeout(debounceRef.current);
    if (!q.trim()) {
      setSuggestions([]);
      setShowSuggestions(false);
      return;
    }
    debounceRef.current = setTimeout(async () => {
      setLoadingSuggestions(true);
      try {
        const data = await tgFetch<ChannelSuggestion[]>(`channels?q=${encodeURIComponent(q)}`);
        setSuggestions(data);
        setShowSuggestions(data.length > 0);
      } catch {
        setSuggestions([]);
      } finally {
        setLoadingSuggestions(false);
      }
    }, 300);
  }, []);

  function handleChannelInput(value: string) {
    setNewChannel(value);
    searchChannels(value);
  }

  function selectSuggestion(s: ChannelSuggestion) {
    const username = s.username ?? s.title;
    setNewChannel(username);
    setSuggestions([]);
    setShowSuggestions(false);
  }

  async function loadAll() {
    try {
      const [s, srcs] = await Promise.all([
        tgFetch<TelegramMonitorStatus>("status").catch(() => null),
        tgFetch<TelegramSourceDto[]>("sources").catch(() => []),
      ]);
      if (s) {
        setStatus(s);
        if (s.phone) setPhone(s.phone);
        if (s.api_id) setApiId(String(s.api_id));
        const interval = String(s.sync_interval_minutes || 30);
        setSyncInterval(SYNC_INTERVAL_VALUES.has(interval) ? interval : "30");
      }
      setSources(srcs);
    } catch { /* ignore */ }
  }

  async function handleSaveSettings() {
    if (!apiId || !apiHash || !phone) return;
    setSavingSettings(true);
    try {
      await tgFetch("settings", {
        method: "POST",
        body: JSON.stringify({ api_id: parseInt(apiId), api_hash: apiHash, phone, sync_interval_minutes: parseInt(syncInterval) }),
      });
      toast(t("settings.savedSuccess"), "success");
      const s = await tgFetch<TelegramMonitorStatus>("status");
      setStatus(s);
    } catch {
      toast(t("settings.savedError"), "error");
    } finally {
      setSavingSettings(false);
    }
  }

  async function handleSendCode() {
    setSendingCode(true);
    try {
      await tgFetch("auth/start", { method: "POST" });
      setCodeSent(true);
      toast(t("telegramMonitor.codeSent"), "success");
    } catch (e) {
      toast(String(e), "error");
    } finally {
      setSendingCode(false);
    }
  }

  async function handleVerify() {
    if (!code) return;
    setVerifying(true);
    try {
      await tgFetch("auth/verify", { method: "POST", body: JSON.stringify({ code }) });
      setCode("");
      setCodeSent(false);
      toast(t("telegramMonitor.authorized"), "success");
      const s = await tgFetch<TelegramMonitorStatus>("status");
      setStatus(s);
    } catch (e) {
      toast(String(e), "error");
    } finally {
      setVerifying(false);
    }
  }

  async function handleDisconnect() {
    setDisconnecting(true);
    try {
      await tgFetch("auth", { method: "DELETE" });
      setCodeSent(false);
      const s = await tgFetch<TelegramMonitorStatus>("status");
      setStatus(s);
    } catch { /* ignore */ } finally {
      setDisconnecting(false);
    }
  }

  async function handleAddSource() {
    if (!newChannel) return;
    setAddingSource(true);
    try {
      const src = await tgFetch<TelegramSourceDto>("sources", {
        method: "POST",
        body: JSON.stringify({ channel_username: newChannel, library_id: newChannelLibrary || null }),
      });
      setSources(prev => [...prev, src]);
      setNewChannel("");
      setNewChannelLibrary("");
    } catch (e) {
      toast(String(e), "error");
    } finally {
      setAddingSource(false);
    }
  }

  async function handleDeleteSource(id: string) {
    try {
      await tgFetch(`sources/${id}`, { method: "DELETE" });
      setSources(prev => prev.filter(s => s.id !== id));
    } catch (e) {
      toast(String(e), "error");
    }
  }

  async function handleSaveSyncInterval(next: string) {
    setSyncInterval(next);
    try {
      await tgFetch("settings", {
        method: "POST",
        body: JSON.stringify({ sync_interval_minutes: parseInt(next) }),
      });
      const s = await tgFetch<TelegramMonitorStatus>("status");
      setStatus(s);
      toast(t("settings.savedSuccess"), "success");
    } catch {
      toast(t("settings.savedError"), "error");
    }
  }

  const authorized = status?.authorized ?? false;

  return (
    <Card className="mb-6">
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          <Icon name="download" size="md" />
          {t("telegramMonitor.title")}
        </CardTitle>
        <CardDescription>{t("telegramMonitor.desc")}</CardDescription>
      </CardHeader>
      <CardContent>
        <div className="space-y-6">

          {/* API credentials */}
          <div className="space-y-3">
            <div className="flex gap-4">
              <FormField className="w-40">
                <label className="text-sm font-medium text-muted-foreground mb-1 block">{t("telegramMonitor.apiId")}</label>
                <FormInput
                  type="text"
                  placeholder={t("telegramMonitor.apiIdPlaceholder")}
                  value={apiId}
                  onChange={e => setApiId(e.target.value)}
                />
              </FormField>
              <FormField className="flex-1">
                <label className="text-sm font-medium text-muted-foreground mb-1 block">{t("telegramMonitor.apiHash")}</label>
                <FormInput
                  type="password"
                  autoComplete="off"
                  placeholder={t("telegramMonitor.apiHashPlaceholder")}
                  value={apiHash}
                  onChange={e => setApiHash(e.target.value)}
                />
              </FormField>
              <FormField className="w-48">
                <label className="text-sm font-medium text-muted-foreground mb-1 block">{t("telegramMonitor.phone")}</label>
                <FormInput
                  type="text"
                  placeholder={t("telegramMonitor.phonePlaceholder")}
                  value={phone}
                  onChange={e => setPhone(e.target.value)}
                />
              </FormField>
            </div>
            <p className="text-xs text-muted-foreground">{t("telegramMonitor.apiHelp")}</p>
            <Button
              onClick={handleSaveSettings}
              disabled={savingSettings || !apiId || !apiHash || !phone}
            >
              {t("telegramMonitor.saveSettings")}
            </Button>
          </div>

          {/* Auth section */}
          {status?.configured && (
            <div className="border-t pt-4 space-y-3">
              <div className="flex items-center gap-3">
                <span className={`text-sm font-medium ${authorized ? "text-success" : "text-muted-foreground"}`}>
                  {authorized ? t("telegramMonitor.authorized") : t("telegramMonitor.notAuthorized")}
                </span>
                {authorized && (
                  <Button
                    variant="ghost"
                    onClick={handleDisconnect}
                    disabled={disconnecting}
                  >
                    {disconnecting ? t("telegramMonitor.disconnecting") : t("telegramMonitor.disconnect")}
                  </Button>
                )}
              </div>

              {!authorized && !codeSent && (
                <Button onClick={handleSendCode} disabled={sendingCode}>
                  {sendingCode ? t("telegramMonitor.sending") : t("telegramMonitor.sendCode")}
                </Button>
              )}

              {!authorized && codeSent && (
                <div className="flex items-end gap-3">
                  <FormField className="w-40">
                    <label className="text-sm font-medium text-muted-foreground mb-1 block">{t("telegramMonitor.enterCode")}</label>
                    <FormInput
                      type="text"
                      placeholder={t("telegramMonitor.codePlaceholder")}
                      value={code}
                      onChange={e => setCode(e.target.value)}
                      onKeyDown={e => e.key === "Enter" && handleVerify()}
                    />
                  </FormField>
                  <Button onClick={handleVerify} disabled={verifying || !code}>
                    {verifying ? t("telegramMonitor.verifying") : t("telegramMonitor.verify")}
                  </Button>
                </div>
              )}
            </div>
          )}

          {/* Sources */}
          {authorized && (
            <div className="border-t pt-4 space-y-3">
              <div className="flex items-end justify-between gap-3">
                <h4 className="text-sm font-semibold">{t("telegramMonitor.sourcesTitle")}</h4>
                <FormField className="ml-auto w-44">
                  <label className="text-sm font-medium text-muted-foreground mb-1 block">{t("telegramMonitor.syncInterval")}</label>
                  <FormSelect
                    value={syncInterval}
                    onChange={e => handleSaveSyncInterval(e.target.value)}
                  >
                    {SYNC_INTERVAL_OPTIONS.map(option => (
                      <option key={option.value} value={option.value}>{option.label}</option>
                    ))}
                  </FormSelect>
                  <p className="text-xs text-muted-foreground mt-1">{t("telegramMonitor.syncIntervalHelp")}</p>
                </FormField>
              </div>

              {sources.length === 0 && (
                <p className="text-sm text-muted-foreground">{t("telegramMonitor.noSources")}</p>
              )}
              {sources.map(src => (
                <div key={src.id} className="flex items-center gap-3 text-sm">
                  <span className="font-mono">@{src.channel_username}</span>
                  {src.channel_title && <span className="text-muted-foreground">{src.channel_title}</span>}
                  <button
                    type="button"
                    onClick={() => handleDeleteSource(src.id)}
                    className="ml-auto text-destructive hover:text-destructive/80 text-xs"
                  >
                    ×
                  </button>
                </div>
              ))}

              {/* Add channel form */}
              <div className="flex items-end gap-3">
                <FormField className="flex-1">
                  <label className="text-sm font-medium text-muted-foreground mb-1 block">{t("telegramMonitor.addChannel")}</label>
                  <div className="relative" ref={autocompleteRef}>
                    <FormInput
                      type="search"
                      placeholder={t("telegramMonitor.channelSearchPlaceholder")}
                      value={newChannel}
                      onChange={e => handleChannelInput(e.target.value)}
                      onFocus={() => { if (suggestions.length > 0) setShowSuggestions(true); }}
                      autoComplete="off"
                      data-lpignore="true"
                      data-1p-ignore
                      data-bwignore
                    />
                    {loadingSuggestions && (
                      <div className="absolute right-2 top-1/2 -translate-y-1/2">
                        <Icon name="spinner" size="sm" className="animate-spin text-muted-foreground" />
                      </div>
                    )}
                    {showSuggestions && suggestions.length > 0 && (
                      <ul className="absolute z-50 mt-1 w-full bg-popover border border-border rounded-md shadow-lg max-h-60 overflow-auto">
                        {suggestions.map((s, i) => (
                          <li
                            key={i}
                            className="flex items-center gap-2 px-3 py-2 cursor-pointer hover:bg-accent text-sm"
                            onMouseDown={e => { e.preventDefault(); selectSuggestion(s); }}
                          >
                            <span className="flex-1 truncate font-medium">{s.title}</span>
                            {s.username && (
                              <span className="text-xs text-muted-foreground font-mono">@{s.username}</span>
                            )}
                            <span className="text-xs text-muted-foreground shrink-0">
                              {s.kind === "channel" ? t("telegramMonitor.channelKindChannel") : t("telegramMonitor.channelKindGroup")}
                            </span>
                          </li>
                        ))}
                      </ul>
                    )}
                  </div>
                </FormField>
                <FormField className="w-48">
                  <label className="text-sm font-medium text-muted-foreground mb-1 block">{t("telegramMonitor.library")}</label>
                  <FormSelect value={newChannelLibrary} onChange={e => setNewChannelLibrary(e.target.value)}>
                    <option value="">{t("telegramMonitor.noLibrary")}</option>
                    {libraries.map(lib => (
                      <option key={lib.id} value={lib.id}>{lib.name}</option>
                    ))}
                  </FormSelect>
                </FormField>
                <Button
                  onClick={handleAddSource}
                  disabled={addingSource || !newChannel}
                >
                  {addingSource ? t("telegramMonitor.adding") : t("telegramMonitor.add")}
                </Button>
              </div>

              {syncResult && (
                <p className="text-xs text-success">
                  {t("telegramMonitor.syncResult")
                    .replace("{{new}}", String(syncResult.new_books))
                    .replace("{{synced}}", String(syncResult.synced))}
                </p>
              )}
            </div>
          )}

          {authorized && sources.length > 0 && (
            <p className="text-xs text-muted-foreground border-t pt-3">
              {t("telegramMonitor.seeDownloadsPage")}
            </p>
          )}
        </div>
      </CardContent>
    </Card>
  );
}
