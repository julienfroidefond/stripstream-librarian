"use client";

import { useState, useCallback } from "react";
import { useRouter } from "next/navigation";
import { Button, Icon, Modal, FormInput } from "./ui";
import { useTranslation } from "@/lib/i18n/context";

interface RenameEntry {
  book_id: string;
  old_filename: string;
  new_filename: string;
  old_path: string;
  new_path: string;
  changed: boolean;
}

interface RenameResponse {
  series_id: string;
  renames: RenameEntry[];
  errors: { book_id: string; filename: string; error: string }[];
  executed: boolean;
}

type ModalStep = "idle" | "loading" | "preview" | "executing" | "done" | "error";

interface RenameSeriesBooksModalProps {
  seriesId: string;
  seriesName: string;
  initialFormat?: string | null;
  initialFormatHs?: string | null;
  children?: (open: () => void) => React.ReactNode;
}

export function RenameSeriesBooksModal({
  seriesId,
  seriesName,
  initialFormat,
  initialFormatHs,
  children,
}: RenameSeriesBooksModalProps) {
  const { t } = useTranslation();
  const router = useRouter();
  const [isOpen, setIsOpen] = useState(false);
  const [step, setStep] = useState<ModalStep>("idle");
  const [format, setFormat] = useState(initialFormat || "{series_name} - T{volume_padded} - {title}");
  const [formatHs, setFormatHs] = useState(initialFormatHs || "{series_name} - HS {volume_padded}");
  const [result, setResult] = useState<RenameResponse | null>(null);
  const [error, setError] = useState<string | null>(null);

  const fetchPreview = useCallback(async (template: string, templateHs: string) => {
    setStep("loading");
    setError(null);
    try {
      const resp = await fetch(`/api/series/${seriesId}/rename-books`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ format: template, format_hs: templateHs, mode: "preview" }),
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
  }, [seriesId]);

  const handleOpen = useCallback(() => {
    setIsOpen(true);
    fetchPreview(format, formatHs);
  }, [format, formatHs, fetchPreview]);

  const handleClose = useCallback(() => {
    setIsOpen(false);
    setStep("idle");
    setResult(null);
    setError(null);
  }, []);

  const handleRefresh = useCallback(() => {
    fetchPreview(format, formatHs);
  }, [format, formatHs, fetchPreview]);

  const handleExecute = useCallback(async () => {
    setStep("executing");
    setError(null);
    try {
      const resp = await fetch(`/api/series/${seriesId}/rename-books`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ format, format_hs: formatHs, mode: "execute" }),
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
  }, [seriesId, format, formatHs]);

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

      <Modal isOpen={isOpen} onClose={handleClose} title={t("rename.modalTitle")} maxWidth="3xl" disableClose={step === "executing"}>
        <div className="p-6 space-y-4">
          {/* Template input */}
          <div className="flex gap-2 items-end">
            <div className="flex-1">
              <label className="text-sm font-medium text-muted-foreground mb-1 block">
                {t("rename.template")}
              </label>
              <FormInput
                value={format}
                onChange={(e) => setFormat(e.target.value)}
                placeholder="{series_name} - T{volume_padded} - {title}"
              />
            </div>
            <Button
              variant="outline"
              size="sm"
              onClick={handleRefresh}
              disabled={step === "loading" || step === "executing"}
              className="shrink-0"
            >
              {step === "loading" ? (
                <Icon name="spinner" size="sm" className="animate-spin" />
              ) : (
                t("rename.refreshPreview")
              )}
            </Button>
          </div>

          {/* HS format input */}
          <div className="flex gap-2 items-end">
            <div className="flex-1">
              <label className="text-sm font-medium text-muted-foreground mb-1 block">
                {t("rename.templateHs")}
              </label>
              <FormInput
                value={formatHs}
                onChange={(e) => setFormatHs(e.target.value)}
                placeholder="{series_name} - HS {volume_padded}"
              />
            </div>
          </div>

          {/* Available variables */}
          <div className="flex flex-wrap gap-1.5">
            {["series_name", "volume", "volume_padded", "title", "authors", "publish_date", "isbn"].map((v) => (
              <code
                key={v}
                className="text-xs px-1.5 py-0.5 bg-muted rounded cursor-pointer hover:bg-muted/80 transition-colors"
                onClick={() => setFormat((prev) => prev + `{${v}}`)}
              >
                {`{${v}}`}
              </code>
            ))}
          </div>

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
                      <th className="text-left px-3 py-2 font-medium text-muted-foreground min-w-[300px]">{t("rename.newFilename")}</th>
                      <th className="text-center px-3 py-2 font-medium text-muted-foreground w-20">{t("rename.changed")}</th>
                    </tr>
                  </thead>
                  <tbody className="divide-y divide-border">
                    {result.renames.map((entry) => (
                      <tr key={entry.book_id} className={entry.changed ? "bg-primary/5" : ""}>
                        <td className="px-3 py-2 font-mono text-xs break-all">{entry.old_filename}</td>
                        <td className="px-3 py-2 font-mono text-xs break-all">{entry.new_filename}</td>
                        <td className="px-3 py-2 text-center">
                          {entry.changed ? (
                            <span className="text-primary font-medium">{t("rename.changed")}</span>
                          ) : (
                            <span className="text-muted-foreground">—</span>
                          )}
                        </td>
                      </tr>
                    ))}
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
