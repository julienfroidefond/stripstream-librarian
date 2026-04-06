"use client";

import { useState, useCallback, useRef } from "react";
import { useRouter } from "next/navigation";
import { Button, Icon, Modal } from "./ui";
import { useTranslation } from "@/lib/i18n/context";

type Library = { id: string; name: string };
type MetadataCandidate = {
  provider: string;
  external_id: string;
  external_url: string | null;
  title: string;
  authors: string[];
  description: string | null;
  total_volumes: number | null;
  cover_url: string | null;
  confidence: number;
  metadata_json: Record<string, unknown>;
};

export function CreateSeriesButton({ libraries }: { libraries: Library[] }) {
  const { t } = useTranslation();
  const router = useRouter();
  const [isOpen, setIsOpen] = useState(false);
  const [libraryId, setLibraryId] = useState(libraries[0]?.id ?? "");
  const [name, setName] = useState("");
  const [candidates, setCandidates] = useState<MetadataCandidate[]>([]);
  const [selected, setSelected] = useState<MetadataCandidate | null>(null);
  const [searching, setSearching] = useState(false);
  const [creating, setCreating] = useState(false);
  const debounceRef = useRef<ReturnType<typeof setTimeout>>(undefined);

  const searchMetadata = useCallback(
    async (q: string) => {
      if (q.length < 2 || !libraryId) {
        setCandidates([]);
        return;
      }
      setSearching(true);
      try {
        const resp = await fetch("/api/metadata/search", {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ library_id: libraryId, series_name: q }),
        });
        if (resp.ok) {
          const data = await resp.json();
          setCandidates(Array.isArray(data) ? data : []);
        }
      } catch {
        setCandidates([]);
      } finally {
        setSearching(false);
      }
    },
    [libraryId],
  );

  function handleNameChange(value: string) {
    setName(value);
    setSelected(null);
    if (debounceRef.current) clearTimeout(debounceRef.current);
    debounceRef.current = setTimeout(() => searchMetadata(value), 500);
  }

  async function handleCreate() {
    if (!name.trim() || !libraryId) return;
    setCreating(true);
    try {
      const body: Record<string, unknown> = {
        library_id: libraryId,
        name: name.trim(),
      };
      if (selected) {
        body.provider = selected.provider;
        body.external_id = selected.external_id;
        body.external_url = selected.external_url;
        body.confidence = selected.confidence;
        body.total_volumes = selected.total_volumes;
        body.metadata_json = {
          ...selected.metadata_json,
          description: selected.description,
          authors: selected.authors,
          cover_url: selected.cover_url,
        };
      }
      const resp = await fetch("/api/series/create", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(body),
      });
      if (resp.ok) {
        const data = await resp.json();
        setIsOpen(false);
        router.push(`/series/${data.series_id}`);
      }
    } catch {
      // error
    } finally {
      setCreating(false);
    }
  }

  function handleClose() {
    setIsOpen(false);
    setName("");
    setCandidates([]);
    setSelected(null);
  }

  return (
    <>
      <Button size="sm" onClick={() => setIsOpen(true)}>
        <Icon name="plus" size="sm" />
        {t("seriesDetail.create")}
      </Button>

      <Modal
        isOpen={isOpen}
        onClose={handleClose}
        maxWidth="lg"
        title={t("seriesDetail.createTitle")}
      >
        <div className="p-6 space-y-4">
          {/* Library selector */}
          <div>
            <label className="block text-sm font-medium text-foreground mb-1">
              {t("seriesDetail.createLibrary")}
            </label>
            <select
              value={libraryId}
              onChange={(e) => {
                setLibraryId(e.target.value);
                setCandidates([]);
                setSelected(null);
              }}
              className="w-full px-3 py-2 rounded-lg border border-border bg-background text-foreground text-sm"
            >
              {libraries.map((lib) => (
                <option key={lib.id} value={lib.id}>
                  {lib.name}
                </option>
              ))}
            </select>
          </div>

          {/* Name input */}
          <div>
            <label className="block text-sm font-medium text-foreground mb-1">
              {t("seriesDetail.createName")}
            </label>
            <input
              type="text"
              value={name}
              onChange={(e) => handleNameChange(e.target.value)}
              placeholder={t("seriesDetail.createSearch")}
              className="w-full px-3 py-2 rounded-lg border border-border bg-background text-foreground text-sm placeholder:text-muted-foreground focus:outline-none focus:ring-2 focus:ring-primary"
              autoFocus
            />
          </div>

          {/* Metadata results */}
          {(searching || candidates.length > 0 || (name.length >= 2 && !searching)) && (
            <div className="max-h-72 overflow-y-auto space-y-1 border border-border rounded-lg p-2">
              {searching && (
                <div className="flex items-center justify-center gap-2 py-4 text-sm text-muted-foreground">
                  <Icon name="spinner" size="sm" className="animate-spin" />
                  {t("seriesDetail.createSearching")}
                </div>
              )}
              {!searching && name.length >= 2 && candidates.length === 0 && (
                <p className="text-sm text-muted-foreground text-center py-4">
                  {t("seriesDetail.createNoResults")}
                </p>
              )}
              {candidates.map((c, i) => (
                <button
                  key={`${c.provider}-${c.external_id}-${i}`}
                  type="button"
                  onClick={() => {
                    setSelected(c);
                    setName(c.title);
                  }}
                  className={`w-full text-left px-3 py-2 rounded-lg text-sm transition-colors flex gap-3 ${
                    selected?.external_id === c.external_id && selected?.provider === c.provider
                      ? "bg-primary/10 border border-primary/30"
                      : "hover:bg-muted"
                  }`}
                >
                  {c.cover_url && (
                    <img
                      src={c.cover_url}
                      alt=""
                      className="w-10 h-14 object-cover rounded shrink-0"
                    />
                  )}
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center gap-2">
                      <span className="font-medium truncate">{c.title}</span>
                      <span className="text-[10px] px-1.5 py-0.5 rounded bg-muted text-muted-foreground shrink-0">
                        {c.provider}
                      </span>
                      <span className="text-xs text-muted-foreground shrink-0">
                        {Math.round(c.confidence * 100)}%
                      </span>
                    </div>
                    <div className="text-xs text-muted-foreground truncate">
                      {c.authors.length > 0 && <span>{c.authors.join(", ")}</span>}
                      {c.total_volumes && (
                        <span className="ml-2">{c.total_volumes} vol.</span>
                      )}
                    </div>
                  </div>
                </button>
              ))}
            </div>
          )}
        </div>

        <div className="flex justify-end gap-2 px-6 pb-6">
          <Button variant="outline" size="sm" onClick={handleClose}>
            {t("common.cancel")}
          </Button>
          {name.trim() && !selected && (
            <Button
              variant="outline"
              size="sm"
              onClick={handleCreate}
              disabled={creating || !name.trim()}
            >
              {t("seriesDetail.createWithoutMetadata")}
            </Button>
          )}
          <Button
            size="sm"
            onClick={handleCreate}
            disabled={creating || !name.trim()}
          >
            {creating ? (
              <Icon name="spinner" size="sm" className="animate-spin" />
            ) : (
              <Icon name="plus" size="sm" />
            )}
            {t("seriesDetail.createConfirm")}
          </Button>
        </div>
      </Modal>
    </>
  );
}
