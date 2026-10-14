"use client";

import { useCallback, useEffect, useState } from "react";
import { Badge, Button, FormInput, Icon, Select, SettingsCard, Switch, toast } from "@/app/components/ui";
import { useTranslation } from "@/lib/i18n/context";
import type { DdlProviderDto } from "@/lib/api";

type TestResult = { success: boolean; message: string };

/** Background-search cadences offered in the interval picker. */
const SEARCH_INTERVALS = [0, 15, 30, 60, 360, 1440];

/**
 * Settings card listing every DDL provider. Each provider can be
 * enabled/disabled, given a custom base URL, and probed with a test query.
 * A shared (global) background-search cadence is configured below the list.
 */
export function DdlProvidersCard() {
  const { t } = useTranslation();
  const [providers, setProviders] = useState<DdlProviderDto[] | null>(null);
  const [loadError, setLoadError] = useState(false);
  const [savingId, setSavingId] = useState<string | null>(null);
  const [testingId, setTestingId] = useState<string | null>(null);
  const [testResults, setTestResults] = useState<Record<string, TestResult>>({});
  const [baseUrls, setBaseUrls] = useState<Record<string, string>>({});
  const [searchInterval, setSearchInterval] = useState<number | null>(null);
  const [savingInterval, setSavingInterval] = useState(false);
  const [searching, setSearching] = useState(false);

  useEffect(() => {
    let cancelled = false;
    fetch("/api/ddl-providers")
      .then((resp) => {
        if (!resp.ok) throw new Error(String(resp.status));
        return resp.json();
      })
      .then((list: DdlProviderDto[]) => {
        if (cancelled) return;
        setProviders(list);
        setBaseUrls(Object.fromEntries(list.map((p) => [p.id, p.base_url])));
      })
      .catch(() => {
        if (!cancelled) setLoadError(true);
      });
    fetch("/api/ddl-providers/search-settings")
      .then((resp) => (resp.ok ? resp.json() : null))
      .then((data: { search_interval_minutes: number } | null) => {
        if (!cancelled && data) setSearchInterval(data.search_interval_minutes);
      })
      .catch(() => {
        /* keep the current value */
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const save = useCallback(
    async (id: string, enabled: boolean, baseUrl?: string) => {
      setSavingId(id);
      try {
        const resp = await fetch(`/api/ddl-providers/${id}/toggle`, {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ enabled, base_url: baseUrl }),
        });
        const data = await resp.json().catch(() => null);
        if (!resp.ok) {
          toast(data?.error ?? `Error ${resp.status}`, "error");
          return;
        }
        const list: DdlProviderDto[] = data;
        setProviders(list);
        setBaseUrls(Object.fromEntries(list.map((p) => [p.id, p.base_url])));
        toast(t("ddlProviders.saved"), "success");
      } catch (e) {
        toast(String(e), "error");
      } finally {
        setSavingId(null);
      }
    },
    [t]
  );

  const runTest = useCallback(
    async (id: string) => {
      setTestingId(id);
      setTestResults((prev) => {
        const next = { ...prev };
        delete next[id];
        return next;
      });
      try {
        const resp = await fetch(`/api/ddl-providers/${id}/test`, {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({}),
        });
        if (!resp.ok) {
          const body = await resp.json().catch(() => ({}));
          setTestResults((prev) => ({
            ...prev,
            [id]: { success: false, message: body?.error ?? `Error ${resp.status}` },
          }));
          return;
        }
        const data: TestResult = await resp.json();
        setTestResults((prev) => ({ ...prev, [id]: data }));
      } catch (e) {
        setTestResults((prev) => ({ ...prev, [id]: { success: false, message: String(e) } }));
      } finally {
        setTestingId(null);
      }
    },
    []
  );

  const saveInterval = useCallback(
    async (minutes: number) => {
      const previous = searchInterval;
      setSearchInterval(minutes);
      setSavingInterval(true);
      try {
        const resp = await fetch("/api/ddl-providers/search-settings", {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ search_interval_minutes: minutes }),
        });
        const data = await resp.json().catch(() => null);
        if (!resp.ok) {
          setSearchInterval(previous);
          toast(data?.error ?? `Error ${resp.status}`, "error");
          return;
        }
        setSearchInterval(data.search_interval_minutes);
      } catch (e) {
        setSearchInterval(previous);
        toast(String(e), "error");
      } finally {
        setSavingInterval(false);
      }
    },
    [searchInterval]
  );

  const runSearchAll = useCallback(async () => {
    setSearching(true);
    try {
      const resp = await fetch("/api/ddl-providers/search-all", { method: "POST" });
      const data = await resp.json().catch(() => null);
      if (!resp.ok) {
        toast(data?.error ?? `Error ${resp.status}`, "error");
        return;
      }
      toast(
        data?.status === "already_running"
          ? t("ddlProviders.searchAlreadyRunning")
          : t("ddlProviders.searchQueued"),
        "success"
      );
    } catch (e) {
      toast(String(e), "error");
    } finally {
      setSearching(false);
    }
  }, [t]);

  const hasEnabledProvider = providers?.some((p) => p.enabled) ?? false;

  return (
    <SettingsCard
      icon="globe"
      title={t("ddlProviders.title")}
      description={t("ddlProviders.desc")}
    >
      {providers === null && !loadError && (
        <p className="flex items-center gap-2 text-sm text-muted-foreground">
          <Icon name="spinner" size="sm" className="animate-spin" />
          {t("ddlProviders.loading")}
        </p>
      )}
      {loadError && <p className="text-sm text-destructive">{t("ddlProviders.loadError")}</p>}
      {providers?.length === 0 && (
        <p className="text-sm text-muted-foreground">{t("ddlProviders.noProviders")}</p>
      )}

      {providers && providers.length > 0 && (
        <ul className="divide-y divide-border">
          {providers.map((provider) => (
            <li key={provider.id} className="py-4 first:pt-0 last:pb-0 space-y-3">
              <div className="flex items-start justify-between gap-4">
                <div className="min-w-0">
                  <div className="flex flex-wrap items-center gap-2">
                    <span className="font-medium text-foreground">{provider.label}</span>
                    <Badge variant="muted">
                      {provider.kind === "html"
                        ? t("ddlProviders.kindHtml")
                        : t("ddlProviders.kindJson")}
                    </Badge>
                    {provider.requires_auth && (
                      <Badge variant="warning">{t("ddlProviders.requiresAuth")}</Badge>
                    )}
                  </div>
                  <p className="mt-1 text-sm text-muted-foreground">{provider.description}</p>
                </div>
                <Switch
                  checked={provider.enabled}
                  disabled={savingId === provider.id}
                  aria-label={provider.label}
                  onChange={(e) => save(provider.id, e.target.checked, baseUrls[provider.id])}
                />
              </div>

              <div className="flex flex-col gap-2 sm:flex-row sm:items-center">
                <FormInput
                  value={baseUrls[provider.id] ?? ""}
                  placeholder={t("ddlProviders.baseUrlPlaceholder")}
                  disabled={savingId === provider.id}
                  onChange={(e) =>
                    setBaseUrls((prev) => ({ ...prev, [provider.id]: e.target.value }))
                  }
                  onBlur={() => {
                    const current = providers.find((p) => p.id === provider.id)?.base_url ?? "";
                    if ((baseUrls[provider.id] ?? "") !== current) {
                      save(provider.id, provider.enabled, baseUrls[provider.id]);
                    }
                  }}
                  className="sm:max-w-md"
                  aria-label={t("ddlProviders.baseUrl")}
                />
                <Button
                  type="button"
                  variant="secondary"
                  size="sm"
                  disabled={testingId === provider.id}
                  onClick={() => runTest(provider.id)}
                >
                  <Icon
                    name={testingId === provider.id ? "spinner" : "play"}
                    size="sm"
                    className={testingId === provider.id ? "animate-spin" : ""}
                  />
                  {testingId === provider.id
                    ? t("ddlProviders.testing")
                    : t("ddlProviders.test")}
                </Button>
              </div>

              {testResults[provider.id] && (
                <p
                  className={`text-sm ${
                    testResults[provider.id].success ? "text-success" : "text-destructive"
                  }`}
                >
                  {testResults[provider.id].message}
                </p>
              )}
            </li>
          ))}
        </ul>
      )}

      <div className="mt-6 border-t border-border pt-4 space-y-3">
        <div>
          <p className="text-sm font-medium text-foreground">
            {t("ddlProviders.backgroundSearch")}
          </p>
          <p className="mt-1 text-sm text-muted-foreground">
            {t("ddlProviders.backgroundSearchDesc")}
          </p>
        </div>
        <div className="flex flex-col gap-2 sm:flex-row sm:items-center">
          <Select
            value={searchInterval ?? 0}
            disabled={savingInterval || searchInterval === null}
            onChange={(e) => saveInterval(Number(e.target.value))}
            className="sm:max-w-xs"
            aria-label={t("ddlProviders.searchInterval")}
          >
            {SEARCH_INTERVALS.map((minutes) => (
              <option key={minutes} value={minutes}>
                {minutes === 0
                  ? t("ddlProviders.intervalOff")
                  : t("ddlProviders.intervalMinutes", { count: minutes })}
              </option>
            ))}
          </Select>
          <Button
            type="button"
            variant="secondary"
            size="sm"
            disabled={searching || !hasEnabledProvider}
            onClick={runSearchAll}
            title={hasEnabledProvider ? undefined : t("ddlProviders.enableFirst")}
          >
            <Icon
              name={searching ? "spinner" : "search"}
              size="sm"
              className={searching ? "animate-spin" : ""}
            />
            {t("ddlProviders.searchNow")}
          </Button>
        </div>
      </div>

      <p className="text-xs text-muted-foreground">{t("ddlProviders.autoImport")}</p>
    </SettingsCard>
  );
}
