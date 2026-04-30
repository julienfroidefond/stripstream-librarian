"use client";

import { useState, useCallback, useEffect } from "react";
import { useRouter, useSearchParams } from "next/navigation";
import { Icon } from "@/app/components/ui";
import { useTranslation } from "@/lib/i18n/context";
import type { DiscoverySuggestion } from "../page";
import { DiscoveryCard } from "./DiscoveryCard";
import { ProwlarrDiscoveryList } from "./ProwlarrDiscoveryList";

interface Library {
  id: string;
  name: string;
  tags: string[];
}

type TabId = "trending" | "prowlarr";

const PROVIDERS = [
  { id: "sc_trending_bd", label: "Nouveautés BD", description: "", hasPeriod: true },
  { id: "sc_trending_manga", label: "Nouveautés Manga", description: "", hasPeriod: true },
  { id: "sc_best_bd", label: "Meilleures BD", description: "", hasPeriod: true },
  { id: "sc_best_manga", label: "Meilleurs Manga", description: "", hasPeriod: true },
  { id: "bedetheque", label: "Bédéthèque", description: "Top BD", hasPeriod: false },
  { id: "senscritique_bd", label: "SensCritique", description: "Top BD", hasPeriod: false },
  { id: "senscritique", label: "SensCritique", description: "Top Manga", hasPeriod: false },
  { id: "anilist", label: "AniList", description: "Top Manga", hasPeriod: false },
];

const PERIODS = [
  { id: "month", label: "Mois" },
  { id: "year", label: "Année" },
  { id: "all", label: "Tous les temps" },
];

export function DiscoveryGrid({
  initialSuggestions,
  libraries,
  provider: initialProvider,
  prowlarrConfigured,
}: {
  initialSuggestions: DiscoverySuggestion[];
  libraries: Library[];
  provider: string;
  prowlarrConfigured: boolean;
}) {
  const { t } = useTranslation();
  const router = useRouter();
  const searchParams = useSearchParams();
  const [suggestions, setSuggestions] = useState(initialSuggestions);
  const [addedIds, setAddedIds] = useState<Set<string>>(new Set());
  const [activeProvider, setActiveProvider] = useState(searchParams.get("provider") || initialProvider);
  const [activeTab, setActiveTab] = useState<TabId>((searchParams.get("tab") as TabId) || "trending");
  const [activePeriod, setActivePeriod] = useState("month");
  const [loading, setLoading] = useState(false);

  const activeProviderDef = PROVIDERS.find((p) => p.id === activeProvider);

  function updateUrl(tab: TabId, provider?: string) {
    const params = new URLSearchParams();
    params.set("tab", tab);
    if (provider) params.set("provider", provider);
    router.replace(`?${params.toString()}`, { scroll: false });
  }

  const [refreshing, setRefreshing] = useState(false);

  const fetchData = useCallback(async (providerId: string, period?: string, nocache = false) => {
    setLoading(true);
    setAddedIds(new Set());
    const provDef = PROVIDERS.find((p) => p.id === providerId);
    const periodParam = provDef?.hasPeriod && period ? `&period=${period}` : "";
    try {
      const resp = await fetch(`/api/discovery/trending?provider=${providerId}&limit=100${periodParam}${nocache ? "&nocache=true" : ""}`);
      if (resp.ok) {
        setSuggestions(await resp.json());
      } else {
        setSuggestions([]);
      }
    } catch {
      setSuggestions([]);
    } finally {
      setLoading(false);
    }
  }, []);

  function handleProviderClick(providerId: string) {
    setActiveProvider(providerId);
    setActiveTab("trending");
    updateUrl("trending", providerId);
    const provDef = PROVIDERS.find((p) => p.id === providerId);
    const period = provDef?.hasPeriod ? activePeriod : undefined;
    fetchData(providerId, period);
  }

  function handlePeriodClick(period: string) {
    setActivePeriod(period);
    fetchData(activeProvider, period);
  }

  async function handleHardRefresh() {
    setRefreshing(true);
    if (activeTab === "prowlarr") {
      setProwlarrKey((k) => k + 1);
      setRefreshing(false);
    } else {
      const period = activeProviderDef?.hasPeriod ? activePeriod : undefined;
      await fetchData(activeProvider, period, true);
      setRefreshing(false);
    }
  }

  const [prowlarrKey, setProwlarrKey] = useState(0);

  function handleAdded(externalId: string) {
    setAddedIds((prev) => new Set(prev).add(externalId));
  }

  // Hidden items management
  type HiddenItem = { id: string; provider: string; external_id: string; title: string; cover_url: string | null };
  const [showHidden, setShowHidden] = useState(false);
  const [hiddenItems, setHiddenItems] = useState<HiddenItem[]>([]);
  const [loadingHidden, setLoadingHidden] = useState(false);

  async function fetchHidden() {
    setLoadingHidden(true);
    try {
      const resp = await fetch("/api/discovery/hidden");
      if (resp.ok) setHiddenItems(await resp.json());
    } catch { /* ignore */ }
    finally { setLoadingHidden(false); }
  }

  async function handleUnhide(item: HiddenItem) {
    await fetch("/api/discovery/unhide", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ provider: item.provider, external_id: item.external_id, title: item.title }),
    });
    setHiddenItems((prev) => prev.filter((h) => h.id !== item.id));
  }

  const visibleSuggestions = suggestions.filter((s) => !addedIds.has(s.external_id));

  const tabs: { id: TabId; label: string }[] = [
    { id: "trending", label: t("discovery.trending") },
    ...(prowlarrConfigured ? [{ id: "prowlarr" as TabId, label: t("discovery.prowlarr") }] : []),
  ];

  return (
    <div className="space-y-4">
      {/* Tabs + refresh */}
      <div className="flex items-center gap-1 border-b border-border">
        {tabs.map((tab) => (
          <button
            key={tab.id}
            onClick={() => { setActiveTab(tab.id); updateUrl(tab.id, activeProvider); }}
            className={`px-4 py-2.5 text-sm font-medium border-b-2 transition-colors -mb-px ${
              activeTab === tab.id
                ? "border-primary text-primary"
                : "border-transparent text-muted-foreground hover:text-foreground hover:border-border"
            }`}
          >
            {tab.label}
          </button>
        ))}
        <div className="ml-auto flex items-center gap-1 mb-px">
          <button
            onClick={() => { setShowHidden(!showHidden); if (!showHidden) fetchHidden(); }}
            className={`px-2 py-1.5 text-sm transition-colors ${showHidden ? "text-primary" : "text-muted-foreground hover:text-foreground"}`}
            title={t("discovery.showHidden")}
          >
            <Icon name="eye" size="sm" />
          </button>
          <button
            onClick={handleHardRefresh}
            disabled={refreshing || loading}
            className="px-2 py-1.5 text-muted-foreground hover:text-foreground transition-colors disabled:opacity-50"
            title={t("discovery.hardRefresh")}
          >
            {refreshing ? (
              <Icon name="spinner" size="sm" className="animate-spin" />
            ) : (
              <Icon name="refresh" size="sm" />
            )}
          </button>
        </div>
      </div>

      {/* Hidden items panel */}
      {showHidden && (
        <div className="border border-border rounded-lg p-4 bg-card">
          <h3 className="text-sm font-semibold text-foreground mb-3">{t("discovery.showHidden")}</h3>
          {loadingHidden ? (
            <div className="flex justify-center py-4"><Icon name="spinner" size="sm" className="animate-spin" /></div>
          ) : hiddenItems.length === 0 ? (
            <p className="text-sm text-muted-foreground">{t("discovery.noHidden")}</p>
          ) : (
            <div className="grid grid-cols-1 sm:grid-cols-2 md:grid-cols-3 gap-2">
              {hiddenItems.map((item) => (
                <div key={item.id} className="flex items-center gap-2 px-3 py-2 rounded-lg bg-muted/50">
                  {item.cover_url && (
                    <img src={item.cover_url} alt="" className="w-8 h-11 object-cover rounded shrink-0" />
                  )}
                  <div className="min-w-0 flex-1">
                    <p className="text-sm font-medium truncate">{item.title}</p>
                    <p className="text-[10px] text-muted-foreground">{item.provider}</p>
                  </div>
                  <button
                    onClick={() => handleUnhide(item)}
                    className="text-xs px-2 py-1 rounded border border-border hover:bg-background transition-colors shrink-0"
                  >
                    {t("discovery.unhide")}
                  </button>
                </div>
              ))}
            </div>
          )}
        </div>
      )}

      {activeTab === "trending" && (
        <>
          {/* Provider selector */}
          <div className="flex flex-wrap gap-2">
            {PROVIDERS.map((p) => (
              <button
                key={p.id}
                onClick={() => handleProviderClick(p.id)}
                className={`px-4 py-2 rounded-lg text-sm font-medium border transition-colors ${
                  activeProvider === p.id
                    ? "bg-primary/15 text-primary border-primary/30"
                    : "bg-card text-muted-foreground border-border hover:border-primary/30"
                }`}
              >
                {p.label}
                {p.description && <span className="ml-1.5 text-xs opacity-60">{p.description}</span>}
                {activeProvider === p.id && !loading && visibleSuggestions.length > 0 && (
                  <span className="ml-1.5 text-xs opacity-50">({visibleSuggestions.length})</span>
                )}
              </button>
            ))}
          </div>

          {/* Period sub-filter for trending providers */}
          {activeProviderDef?.hasPeriod && (
            <div className="flex items-center gap-1.5">
              {PERIODS.map((p) => (
                <button
                  key={p.id}
                  onClick={() => handlePeriodClick(p.id)}
                  disabled={loading}
                  className={`px-3 py-1.5 rounded-md text-xs font-medium border transition-colors ${
                    activePeriod === p.id
                      ? "bg-primary/15 text-primary border-primary/30"
                      : "bg-card text-muted-foreground border-border hover:border-primary/30"
                  } disabled:opacity-50`}
                >
                  {p.label}
                </button>
              ))}
            </div>
          )}

          {loading ? (
            <div className="flex justify-center py-12">
              <Icon name="spinner" size="lg" className="animate-spin text-muted-foreground" />
            </div>
          ) : visibleSuggestions.length === 0 ? (
            <div className="text-center py-12 text-muted-foreground">
              {t("discovery.noResults")}
            </div>
          ) : (
            <div className="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6 gap-3">
              {visibleSuggestions.map((suggestion) => (
                <DiscoveryCard
                  key={`${activeProvider}-${suggestion.external_id}`}
                  suggestion={suggestion}
                  libraries={libraries}
                  onAdded={handleAdded}
                  onHidden={(id) => setSuggestions((prev) => prev.filter((s) => s.external_id !== id))}
                />
              ))}
            </div>
          )}
        </>
      )}

      {activeTab === "prowlarr" && <ProwlarrDiscoveryList key={prowlarrKey} libraries={libraries} nocache={prowlarrKey > 0} />}
    </div>
  );
}
