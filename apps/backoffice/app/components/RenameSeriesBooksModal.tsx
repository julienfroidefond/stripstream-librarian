"use client";

import { useState, useCallback } from "react";
import { useRouter } from "next/navigation";
import { Button, Icon, Modal, FormInput } from "./ui";
import { useTranslation } from "@/lib/i18n/context";
import type { TranslationKey } from "@/lib/i18n";

interface RenameEntry {
  book_id: string;
  old_filename: string;
  new_filename: string;
  old_path: string;
  new_path: string;
  changed: boolean;
  volume: number | null;
  volume_type: string;
}

interface RenameResponse {
  series_id: string;
  renames: RenameEntry[];
  errors: { book_id: string; filename: string; error: string }[];
  executed: boolean;
}

type ModalStep = "idle" | "loading" | "preview" | "executing" | "done" | "error";

type TemplateKey = "regular" | "hs" | "int" | "oneshot";

interface BookOverride {
  volume?: number;
  volume_type?: string;
}

const VOLUME_TYPES = ["regular", "hs", "integral", "oneshot"] as const;

const VOLUME_TYPE_LABEL: Record<(typeof VOLUME_TYPES)[number], TranslationKey> = {
  regular: "volumeType.regular",
  hs: "volumeType.hs",
  integral: "volumeType.integral",
  oneshot: "volumeType.oneshot",
};

const TEMPLATE_FIELDS: { key: TemplateKey; label: TranslationKey }[] = [
  { key: "regular", label: "rename.template" },
  { key: "hs", label: "rename.templateHs" },
  { key: "int", label: "rename.templateInt" },
  { key: "oneshot", label: "rename.templateOneshot" },
];

const AVAILABLE_VARS = [
  "series_name",
  "volume",
  "volume_padded",
  "title",
  "authors",
  "publish_date",
  "isbn",
];

interface RenameSeriesBooksModalProps {
  seriesId: string;
  seriesName: string;
  initialFormat?: string | null;
  initialFormatHs?: string | null;
  initialFormatInt?: string | null;
  initialFormatOneshot?: string | null;
  children?: (open: () => void) => React.ReactNode;
}

export function RenameSeriesBooksModal({
  seriesId,
  seriesName,
  initialFormat,
  initialFormatHs,
  initialFormatInt,
  initialFormatOneshot,
  children,
}: RenameSeriesBooksModalProps) {
  const { t } = useTranslation();
  const router = useRouter();
  const [isOpen, setIsOpen] = useState(false);
  const [step, setStep] = useState<ModalStep>("idle");
  const [formats, setFormats] = useState<Record<TemplateKey, string>>({
    regular: initialFormat || "{series_name} - T{volume_padded} - {title}",
    hs: initialFormatHs || "{series_name} - HS {volume_padded}",
    int: initialFormatInt || "{series_name} - INT {volume_padded}",
    oneshot: initialFormatOneshot || "{series_name}",
  });
  const [activeTemplate, setActiveTemplate] = useState<TemplateKey>("regular");
  const [showTemplates, setShowTemplates] = useState(false);
  const [overrides, setOverrides] = useState<Record<string, BookOverride>>({});
  const [result, setResult] = useState<RenameResponse | null>(null);
  const [error, setError] = useState<string | null>(null);

  const buildBody = useCallback(
    (mode: "preview" | "execute", ovs: Record<string, BookOverride>) => ({
      format: formats.regular,
      format_hs: formats.hs,
      format_int: formats.int,
      format_oneshot: formats.oneshot,
      mode,
      overrides: Object.entries(ovs).map(([book_id, o]) => ({
        book_id,
        volume: o.volume ?? null,
        volume_type: o.volume_type ?? null,
      })),
    }),
    [formats],
  );

  const fetchPreview = useCallback(
    async (ovs: Record<string, BookOverride>) => {
      setStep("loading");
      setError(null);
      try {
        const resp = await fetch(`/api/series/${seriesId}/rename-books`, {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify(buildBody("preview", ovs)),
        });
        if (!resp.ok) {
          let msg = `Error ${resp.status}`;
          try {
            const errData = await resp.json();
            msg = errData?.error || msg;
          } catch { /* non-JSON response */ }
          setError(msg);
          setStep("error");
          return;
        }
        const data: RenameResponse = await resp.json();
        setResult(data);
        setStep("preview");
      } catch (e) {
        setError(e instanceof Error ? e.message : "Network error");
        setStep("error");
      }
    },
    [seriesId, buildBody],
  );

  const handleOpen = useCallback(() => {
    setIsOpen(true);
    setOverrides({});
    fetchPreview({});
  }, [fetchPreview]);

  const handleClose = useCallback(() => {
    setIsOpen(false);
    setStep("idle");
    setResult(null);
    setOverrides({});
    setError(null);
  }, []);

  const handleRefresh = useCallback(() => {
    fetchPreview(overrides);
  }, [overrides, fetchPreview]);

  const updateOverride = useCallback((bookId: string, patch: BookOverride) => {
    setOverrides((prev) => ({ ...prev, [bookId]: { ...prev[bookId], ...patch } }));
  }, []);

  const handleVolumeBlur = useCallback(() => {
    fetchPreview(overrides);
  }, [overrides, fetchPreview]);

  const handleTypeChange = useCallback(
    (bookId: string, volumeType: string) => {
      const next = { ...overrides, [bookId]: { ...overrides[bookId], volume_type: volumeType } };
      setOverrides(next);
      fetchPreview(next);
    },
    [overrides, fetchPreview],
  );

  const appendVar = useCallback((name: string) => {
    setFormats((prev) => ({ ...prev, [activeTemplate]: prev[activeTemplate] + `{${name}}` }));
  }, [activeTemplate]);

  const handleExecute = useCallback(async () => {
    setStep("executing");
    setError(null);
    try {
      const resp = await fetch(`/api/series/${seriesId}/rename-books`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(buildBody("execute", overrides)),
      });
      const data: RenameResponse = await resp.json();
      if (!resp.ok) {
        setError((data as unknown as { error?: string }).error || "Rename failed");
        setStep("error");
        return;
      }
      setResult(data);
      setStep("done");
    } catch {
      setError("Network error");
      setStep("error");
    }
  }, [seriesId, buildBody, overrides]);

  const changedCount = result?.renames.filter((r) => r.changed).length ?? 0;

  return (
    <>
      {children ? (
        children(handleOpen)
      ) : (
        <button
          type="button"
          onClick={handleOpen}
          className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg border border-border bg-card text-foreground text-sm font-medium hover:bg-muted transition-colors"
        >
          <Icon name="edit" size="sm" />
          {t("rename.button")}
        </button>
      )}

      <Modal isOpen={isOpen} onClose={handleClose} title={t("rename.modalTitle")} maxWidth="5xl" disableClose={step === "executing"}>
        <div className="p-6 space-y-4">
          {/* Templates (collapsible) */}
          <div className="border border-border rounded-lg">
            <button
              type="button"
              onClick={() => setShowTemplates((v) => !v)}
              className="w-full flex items-center justify-between px-3 py-2 text-sm font-medium text-foreground hover:bg-muted/50 transition-colors"
            >
              <span>{t("rename.templates")}</span>
              <span className="text-muted-foreground">{showTemplates ? "▾" : "▸"}</span>
            </button>
            {showTemplates && (
              <div className="px-3 pb-3 space-y-3 border-t border-border pt-3">
                {TEMPLATE_FIELDS.map((tpl) => (
                  <div key={tpl.key}>
                    <label className="text-xs font-medium text-muted-foreground mb-1 block">
                      {t(tpl.label)}
                    </label>
                    <FormInput
                      value={formats[tpl.key]}
                      onFocus={() => setActiveTemplate(tpl.key)}
                      onChange={(e) => setFormats((prev) => ({ ...prev, [tpl.key]: e.target.value }))}
                    />
                  </div>
                ))}
                <div className="flex flex-wrap gap-1.5">
                  {AVAILABLE_VARS.map((v) => (
                    <code
                      key={v}
                      className="text-xs px-1.5 py-0.5 bg-muted rounded cursor-pointer hover:bg-muted/80 transition-colors"
                      onClick={() => appendVar(v)}
                    >
                      {`{${v}}`}
                    </code>
                  ))}
                </div>
                <div className="flex justify-end">
                  <Button
                    variant="outline"
                    size="sm"
                    onClick={handleRefresh}
                    disabled={step === "loading" || step === "executing"}
                  >
                    {step === "loading" ? (
                      <Icon name="spinner" size="sm" className="animate-spin" />
                    ) : (
                      t("rename.refreshPreview")
                    )}
                  </Button>
                </div>
              </div>
            )}
          </div>

          {/* Hint */}
          <p className="text-xs text-muted-foreground">{t("rename.overrideHint")}</p>

          {/* Loading */}
          {step === "loading" && (
            <div className="flex justify-center py-8">
              <Icon name="spinner" size="lg" className="animate-spin text-muted-foreground" />
            </div>
          )}

          {/* Error */}
          {step === "error" && error && (
            <div className="p-3 rounded-lg bg-destructive/10 text-destructive text-sm">
              {error}
            </div>
          )}

          {/* Preview table */}
          {(step === "preview" || step === "executing" || step === "done") && result && (
            <>
              {changedCount === 0 && step === "preview" && (
                <div className="p-3 rounded-lg bg-muted/50 text-muted-foreground text-sm text-center">
                  {t("rename.noChanges")}
                </div>
              )}

              <div className="max-h-[50vh] overflow-auto border border-border rounded-lg">
                <table className="w-full text-sm">
                  <thead className="bg-muted/50 sticky top-0">
                    <tr>
                      <th className="text-left px-3 py-2 font-medium text-muted-foreground">{t("rename.currentFilename")}</th>
                      <th className="text-left px-3 py-2 font-medium text-muted-foreground min-w-[240px]">{t("rename.newFilename")}</th>
                      <th className="text-left px-3 py-2 font-medium text-muted-foreground w-32">{t("rename.volumeType")}</th>
                      <th className="text-left px-3 py-2 font-medium text-muted-foreground w-20">{t("rename.volume")}</th>
                    </tr>
                  </thead>
                  <tbody className="divide-y divide-border">
                    {result.renames.map((entry) => {
                      const ov = overrides[entry.book_id] ?? {};
                      const effType = ov.volume_type ?? entry.volume_type;
                      const effVolume =
                        ov.volume !== undefined
                          ? String(ov.volume)
                          : entry.volume != null
                            ? String(entry.volume)
                            : "";
                      return (
                        <tr key={entry.book_id} className={entry.changed ? "bg-primary/5" : ""}>
                          <td className="px-3 py-2 font-mono text-xs break-all">{entry.old_filename}</td>
                          <td className="px-3 py-2 font-mono text-xs break-all">{entry.new_filename}</td>
                          <td className="px-3 py-2">
                            <select
                              aria-label={`${t("rename.volumeType")} ${entry.old_filename}`}
                              className="w-full rounded-md border border-border bg-card px-2 py-1 text-xs"
                              value={effType}
                              disabled={step === "executing"}
                              onChange={(e) => handleTypeChange(entry.book_id, e.target.value)}
                            >
                              {VOLUME_TYPES.map((vt) => (
                                <option key={vt} value={vt}>
                                  {t(VOLUME_TYPE_LABEL[vt])}
                                </option>
                              ))}
                            </select>
                          </td>
                          <td className="px-3 py-2">
                            <FormInput
                              type="number"
                              className="w-full px-2 py-1 text-xs"
                              aria-label={`${t("rename.volume")} ${entry.old_filename}`}
                              value={effVolume}
                              disabled={step === "executing" || effType === "oneshot"}
                              onChange={(e) => {
                                const raw = e.target.value;
                                updateOverride(entry.book_id, {
                                  volume: raw === "" ? undefined : parseInt(raw, 10),
                                });
                              }}
                              onBlur={handleVolumeBlur}
                            />
                          </td>
                        </tr>
                      );
                    })}
                  </tbody>
                </table>
              </div>

              {/* Errors from execution */}
              {result.errors.length > 0 && (
                <div className="p-3 rounded-lg bg-destructive/10 text-destructive text-sm">
                  <p className="font-medium mb-1">{t("rename.error")}</p>
                  <ul className="list-disc list-inside">
                    {result.errors.map((err) => (
                      <li key={err.book_id}>{err.filename}: {err.error}</li>
                    ))}
                  </ul>
                </div>
              )}

              {/* Done message */}
              {step === "done" && (
                <div className="p-3 rounded-lg bg-green-500/10 text-green-600 text-sm">
                  {t("rename.success", { count: changedCount })}
                </div>
              )}
            </>
          )}
        </div>

        {/* Footer */}
        <div className="flex items-center justify-end gap-2 px-6 pb-6">
          {step === "done" ? (
            <Button
              variant="default"
              onClick={() => {
                handleClose();
                router.refresh();
              }}
            >
              {t("common.close")}
            </Button>
          ) : (
            <>
              <Button variant="outline" onClick={handleClose} disabled={step === "executing"}>
                {t("common.cancel")}
              </Button>
              {step === "preview" && changedCount > 0 && (
                <Button variant="outline" className="bg-primary text-primary-foreground ring-primary hover:bg-primary/90" onClick={handleExecute}>
                  {t("rename.apply")}
                </Button>
              )}
              {step === "executing" && (
                <Button variant="outline" className="bg-primary text-primary-foreground ring-primary" disabled>
                  <Icon name="spinner" size="sm" className="animate-spin mr-2" />
                  {t("rename.executing")}
                </Button>
              )}
            </>
          )}
        </div>
      </Modal>
    </>
  );
}
