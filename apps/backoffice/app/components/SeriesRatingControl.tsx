"use client";

import { useState } from "react";
import { useRouter } from "next/navigation";
import RatingStars from "./RatingStars";

interface SeriesRatingControlProps {
  seriesId: string;
  initialRating: number | null;
  hasAniListLink: boolean;
}

export function SeriesRatingControl({
  seriesId,
  initialRating,
  hasAniListLink,
}: SeriesRatingControlProps) {
  const [rating, setRating] = useState<number | null>(initialRating);
  const [saving, setSaving] = useState(false);
  const [toast, setToast] = useState<string | null>(null);
  const router = useRouter();

  async function handleChange(newRating: number) {
    setSaving(true);
    setToast(null);
    try {
      const res = await fetch(`/api/series/${seriesId}/rating`, {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ rating: newRating }),
      });
      if (!res.ok) throw new Error(await res.text());
      setRating(newRating);
      if (hasAniListLink) {
        setToast("Synchronisé avec AniList");
        setTimeout(() => setToast(null), 2500);
      }
      router.refresh();
    } catch {
      setToast("Erreur");
      setTimeout(() => setToast(null), 2500);
    } finally {
      setSaving(false);
    }
  }

  async function handleClear() {
    setSaving(true);
    try {
      const res = await fetch(`/api/series/${seriesId}/rating`, { method: "DELETE" });
      if (!res.ok) throw new Error(await res.text());
      setRating(null);
      router.refresh();
    } catch {
      setToast("Erreur");
      setTimeout(() => setToast(null), 2500);
    } finally {
      setSaving(false);
    }
  }

  return (
    <div className="flex items-center gap-2">
      <div
        className={saving ? "opacity-40 pointer-events-none" : ""}
        title={!rating ? "Cliquer pour noter" : undefined}
      >
        <RatingStars
          value={rating}
          onChange={handleChange}
          onClear={handleClear}
          size="lg"
        />
      </div>
      {rating && !saving && (
        <span className="text-sm font-medium text-primary tabular-nums">
          {(rating / 2).toFixed(1)}
        </span>
      )}
      {saving && (
        <svg className="w-3.5 h-3.5 animate-spin text-muted-foreground" fill="none" viewBox="0 0 24 24">
          <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
          <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" />
        </svg>
      )}
      {toast && (
        <span className="text-xs text-green-400">{toast}</span>
      )}
    </div>
  );
}
