"use client";

import { useState } from "react";
import { Button, Icon } from "@/app/components/ui";
import { useTranslation } from "@/lib/i18n/context";
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
}: {
  suggestion: DiscoverySuggestion;
  libraries: Library[];
  onAdded: (externalId: string) => void;
}) {
  const { t } = useTranslation();
  const [adding, setAdding] = useState(false);
  const [added, setAdded] = useState(false);
  const [showLibraryPicker, setShowLibraryPicker] = useState(false);
  const [imgError, setImgError] = useState(false);

  async function handleAdd(libraryId: string) {
    setAdding(true);
    try {
      const resp = await fetch("/api/discovery/add-to-library", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          library_id: libraryId,
          provider: suggestion.provider,
          external_id: suggestion.external_id,
          title: suggestion.title,
          description: suggestion.description,
          authors: suggestion.authors,
          publishers: [],
          genres: suggestion.genres,
          start_year: suggestion.start_year,
          total_volumes: suggestion.total_volumes,
          status: suggestion.status,
          cover_url: suggestion.cover_url,
          external_url: suggestion.external_url,
        }),
      });
      if (resp.ok) {
        setAdded(true);
        onAdded(suggestion.external_id);
      }
    } finally {
      setAdding(false);
      setShowLibraryPicker(false);
    }
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
        {added ? (
          <div className="text-center text-[11px] text-green-600 font-medium py-1">
            {t("discovery.added")}
          </div>
        ) : showLibraryPicker ? (
          <div className="space-y-0.5">
            {libraries.map((lib) => (
              <button
                key={lib.id}
                onClick={() => handleAdd(lib.id)}
                disabled={adding}
                className="w-full text-left text-[11px] px-2 py-1 rounded hover:bg-muted transition-colors disabled:opacity-50"
              >
                {lib.name}
              </button>
            ))}
          </div>
        ) : (
          <Button
            variant="outline"
            size="xs"
            className="w-full text-[11px]"
            onClick={() => {
              if (libraries.length === 1) {
                handleAdd(libraries[0].id);
              } else {
                setShowLibraryPicker(true);
              }
            }}
            disabled={adding}
          >
            {adding ? (
              <>
                <Icon name="spinner" size="sm" className="animate-spin mr-1" />
                {t("discovery.adding")}
              </>
            ) : (
              t("discovery.addToLibrary")
            )}
          </Button>
        )}
      </div>
    </div>
  );
}
