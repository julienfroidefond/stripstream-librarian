"use client";

import { useState, useCallback } from "react";
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
  { id: "anilist", label: "AniList", description: "Manga" },
  { id: "bedetheque", label: "Bédéthèque", description: "BD franco-belge" },
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
  const [suggestions, setSuggestions] = useState(initialSuggestions);
  const [addedIds, setAddedIds] = useState<Set<string>>(new Set());
  const [activeProvider, setActiveProvider] = useState(initialProvider);
  const [activeTab, setActiveTab] = useState<TabId>("trending");
  const [loading, setLoading] = useState(false);

  const fetchProvider = useCallback(async (providerId: string) => {
    setActiveProvider(providerId);
    setActiveTab("trending");
    setLoading(true);
    setAddedIds(new Set());
    try {
      const resp = await fetch(`/api/discovery/trending?provider=${providerId}&limit=30`);
      if (resp.ok) {
        const data = await resp.json();
        setSuggestions(data);
      } else {
        setSuggestions([]);
      }
    } catch {
      setSuggestions([]);
    } finally {
      setLoading(false);
    }
  }, []);

  function handleAdded(externalId: string) {
    setAddedIds((prev) => new Set(prev).add(externalId));
  }

  const visibleSuggestions = suggestions.filter((s) => !addedIds.has(s.external_id));

  const tabs: { id: TabId; label: string }[] = [
    { id: "trending", label: t("discovery.trending") },
    ...(prowlarrConfigured ? [{ id: "prowlarr" as TabId, label: t("discovery.prowlarr") }] : []),
  ];

  return (
    <div className="space-y-4">
      {/* Tabs */}
      <div className="flex gap-1 border-b border-border">
        {tabs.map((tab) => (
          <button
            key={tab.id}
            onClick={() => setActiveTab(tab.id)}
            className={`px-4 py-2.5 text-sm font-medium border-b-2 transition-colors -mb-px ${
              activeTab === tab.id
                ? "border-primary text-primary"
                : "border-transparent text-muted-foreground hover:text-foreground hover:border-border"
            }`}
          >
            {tab.label}
          </button>
        ))}
      </div>

      {activeTab === "trending" && (
        <>
          {/* Provider selector */}
          <div className="flex flex-wrap gap-2">
            {PROVIDERS.map((p) => (
              <button
                key={p.id}
                onClick={() => fetchProvider(p.id)}
                className={`px-4 py-2 rounded-lg text-sm font-medium border transition-colors ${
                  activeProvider === p.id
                    ? "bg-primary/15 text-primary border-primary/30"
                    : "bg-card text-muted-foreground border-border hover:border-primary/30"
                }`}
              >
                {p.label}
                <span className="ml-1.5 text-xs opacity-60">{p.description}</span>
              </button>
            ))}
          </div>

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
                />
              ))}
            </div>
          )}
        </>
      )}

      {activeTab === "prowlarr" && <ProwlarrDiscoveryList />}
    </div>
  );
}
