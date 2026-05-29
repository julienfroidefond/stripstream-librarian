"use client";

import { useState } from "react";
import { Icon } from "@/app/components/ui";
import { useTranslation } from "@/lib/i18n/context";
import { SeriesAddModal } from "./SeriesAddModal";
import type { DiscoverySuggestion } from "../page";

interface Library {
  id: string;
  name: string;
  tags: string[];
}

export function DiscoveryCard({
  suggestion,
  libraries,
  onAdded,
  onHidden,
}: {
  suggestion: DiscoverySuggestion;
  libraries: Library[];
  onAdded: (externalId: string) => void;
  onHidden?: (externalId: string) => void;
}) {
  const { t } = useTranslation();
  const [added, setAdded] = useState(false);
  const [hidden, setHidden] = useState(false);
  const [imgError, setImgError] = useState(false);

  async function handleHide() {
    try {
      const resp = await fetch("/api/discovery/hide", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          provider: suggestion.provider,
          external_id: suggestion.external_id,
          title: suggestion.title,
          cover_url: suggestion.cover_url,
        }),
      });
      if (resp.ok) {
        setHidden(true);
        onHidden?.(suggestion.external_id);
      }
    } catch { /* ignore */ }
  }

  const hasImage = suggestion.cover_url && !imgError;

  return (
    <div className="group relative rounded-xl overflow-hidden border border-border bg-card shadow-soft hover:shadow-card transition-all">
      {/* Cover — fixed aspect ratio */}
      <div className="relative aspect-[2/3] bg-muted">
        {suggestion.cover_url && !imgError ? (
          <img
            src={suggestion.cover_url}
            alt={suggestion.title}
            className="w-full h-full object-cover"
            onError={() => setImgError(true)}
            loading="lazy"
          />
        ) : (
          <div className="w-full h-full flex items-center justify-center text-muted-foreground/40">
            <Icon name="books" size="xl" />
          </div>
        )}

        {/* Gradient overlay at bottom for text readability */}
        {hasImage && (
          <div className="absolute inset-x-0 bottom-0 h-24 bg-gradient-to-t from-black/70 to-transparent pointer-events-none" />
        )}

        {/* Description overlay on hover */}
        {suggestion.description && (
          <div className="absolute inset-0 bg-black/80 opacity-0 group-hover:opacity-100 transition-opacity duration-200 p-3 flex flex-col justify-center overflow-hidden">
            <p className="text-xs text-white/90 leading-relaxed line-clamp-[8]">
              {suggestion.description}
            </p>
          </div>
        )}

        {/* Status badge */}
        {suggestion.status && (
          <span className={`absolute top-2 right-2 text-[10px] px-1.5 py-0.5 rounded-full font-medium backdrop-blur-sm ${
            suggestion.status === "RELEASING" ? "bg-blue-500/70 text-white" :
            suggestion.status === "FINISHED" ? "bg-green-500/70 text-white" :
            suggestion.status === "HIATUS" ? "bg-amber-500/70 text-white" :
            "bg-black/40 text-white"
          }`}>
            {suggestion.status.toLowerCase()}
          </span>
        )}

        {/* Title + genres overlay on image */}
        <div className="absolute inset-x-0 bottom-0 p-2.5">
          <h3 className={`font-semibold text-sm leading-tight line-clamp-2 ${
            hasImage ? "text-white" : "text-foreground"
          }`}>
            {suggestion.title}
          </h3>
          {suggestion.genres.length > 0 && (
            <div className="flex flex-wrap gap-1 mt-1">
              {suggestion.genres.slice(0, 2).map((genre) => (
                <span key={genre} className={`text-[9px] px-1.5 py-0.5 rounded-full ${
                  hasImage
                    ? "bg-white/20 text-white/90 backdrop-blur-sm"
                    : "bg-muted text-muted-foreground"
                }`}>
                  {genre}
                </span>
              ))}
            </div>
          )}
        </div>
      </div>

      {/* Compact info + action */}
      <div className="p-2.5 space-y-2">
        {suggestion.authors.length > 0 && (
          <p className="text-[11px] text-muted-foreground line-clamp-1">
            {suggestion.authors.join(", ")}
          </p>
        )}

        {/* Action */}
        {hidden ? (
          <div className="text-center text-[11px] text-muted-foreground font-medium py-1">
            {t("discovery.hidden")}
          </div>
        ) : added ? (
          <div className="text-center text-[11px] text-green-600 font-medium py-1">
            {t("discovery.added")}
          </div>
        ) : (
          <div className="flex gap-1.5">
            <SeriesAddModal
              mode="discovery"
              suggestion={suggestion}
              libraries={libraries}
              onAdded={() => { setAdded(true); onAdded(suggestion.external_id); }}
            >
              {(open) => (
                <button
                  type="button"
                  className="flex-1 inline-flex items-center justify-center gap-1.5 px-3 py-1.5 rounded-lg bg-primary/10 text-primary text-xs font-medium border border-primary/20 hover:bg-primary/20 transition-colors"
                  onClick={open}
                >
                  <Icon name="plus" size="sm" />
                  {t("discovery.add")}
                </button>
              )}
            </SeriesAddModal>
            <button
              type="button"
              onClick={handleHide}
              title={t("discovery.hide")}
              className="px-2 py-1.5 rounded-lg border border-border text-muted-foreground/60 hover:text-foreground hover:bg-muted transition-colors"
            >
              <Icon name="x" size="sm" />
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
