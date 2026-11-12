"use client";

import { useState } from "react";
import type { OrphanDeleteResultDto, OrphanSeriesItemDto } from "@/lib/api";
import { Badge } from "@/app/components/ui/Badge";
import { Button } from "@/app/components/ui/Button";
import { Icon } from "@/app/components/ui/Icon";
import { Modal } from "@/app/components/ui/Modal";
import { toast } from "@/app/components/ui/Toast";
import { browserApi } from "@/lib/api/browser";
import { useTranslation } from "@/lib/i18n/context";

interface Props {
  series: OrphanSeriesItemDto[];
  /** Called with the ids the user just deleted so the parent can refresh. */
  onDeleted: (ids: string[]) => void;
}

export function OrphansTable({ series, onDeleted }: Props) {
  const { t } = useTranslation();
  const [pendingDelete, setPendingDelete] = useState<OrphanSeriesItemDto | null>(null);
  const [bulkOpen, setBulkOpen] = useState(false);
  const [busy, setBusy] = useState(false);

  const deletable = series.filter((s) => !s.has_active_download);
  const activeDownloadCount = series.length - deletable.length;
  const hasWishlistWarning = series.some((s) => s.has_wishlist_release);
  const hasActiveDownloadWarning = activeDownloadCount > 0;

  async function remove(ids: string[]) {
    if (ids.length === 0) return;
    setBusy(true);
    try {
      const result = await browserApi.series.deleteOrphans(ids);
      const failedIds = new Set(result.failed);
      const removedIds = ids.filter((id) => !failedIds.has(id));

      if (result.deleted > 0 || result.skipped > 0) {
        const key =
          result.skipped > 0
            ? "settings.orphans.deletedWithSkipped"
            : "settings.orphans.deleted";
        toast(
          t(key, {
            count: result.deleted,
            skipped: result.skipped,
            plural: result.deleted !== 1 ? "s" : "",
          }),
          "success",
        );
      }
      if (result.failed.length > 0) {
        toast(
          t("settings.orphans.deletePartial", {
            count: result.failed.length,
            plural: result.failed.length !== 1 ? "s" : "",
          }),
          "error",
        );
      }

      // Refresh even after a partial failure so the table matches the database.
      if (removedIds.length > 0) onDeleted(removedIds);
      setPendingDelete(null);
      setBulkOpen(false);
    } catch {
      toast(t("settings.orphans.deleteError"), "error");
    } finally {
      setBusy(false);
    }
  }

  return (
    <>
      <div className="flex items-center justify-end border-b border-border/50 px-4 py-3">
        <Button
          variant="destructive"
          size="sm"
          onClick={() => setBulkOpen(true)}
          disabled={busy || deletable.length === 0}
        >
          <Icon name="trash" size="sm" className="mr-2" />
          {t("settings.orphans.deletePage")}
        </Button>
      </div>

      <div className="overflow-x-auto">
        <table className="w-full text-sm">
          <thead>
            <tr className="border-b border-border text-muted-foreground">
              <th className="text-left px-4 py-3 text-xs font-medium text-muted-foreground">
                {t("settings.orphans.colName")}
              </th>
              <th className="text-left px-4 py-3 text-xs font-medium text-muted-foreground hidden sm:table-cell">
                {t("settings.orphans.colLibrary")}
              </th>
              <th className="text-left px-4 py-3 text-xs font-medium text-muted-foreground">
                {t("settings.orphans.colProvider")}
              </th>
              <th className="text-left px-4 py-3 text-xs font-medium text-muted-foreground hidden sm:table-cell">
                {t("settings.orphans.colMetadata")}
              </th>
              <th className="text-right px-4 py-3 text-xs font-medium text-muted-foreground hidden md:table-cell">
                {t("settings.orphans.colAge")}
              </th>
              <th className="text-right px-4 py-3 text-xs font-medium text-muted-foreground">
                {t("settings.orphans.colActions")}
              </th>
            </tr>
          </thead>
          <tbody>
            {series.map((item) => (
              <tr key={item.id} className="border-b border-border/50 hover:bg-muted/40 transition-colors">
                <td className="px-4 py-3">
                  <div className="flex flex-wrap items-center gap-2">
                    <span className="font-medium text-foreground">{item.name}</span>
                    {item.has_wishlist_release && (
                      <Badge variant="warning" className="gap-1">
                        <Icon name="warning" size="sm" />
                        {t("settings.orphans.wishlistRelease")}
                      </Badge>
                    )}
                    {item.has_active_download && (
                      <Badge variant="secondary" className="gap-1">
                        <Icon name="download" size="sm" />
                        {t("settings.orphans.activeDownload")}
                      </Badge>
                    )}
                  </div>
                </td>
                <td className="px-4 py-3 text-muted-foreground hidden sm:table-cell">
                  {item.library_name ?? "—"}
                </td>
                <td className="px-4 py-3">
                  {item.metadata_provider ? (
                    <Badge variant="secondary">{item.metadata_provider}</Badge>
                  ) : (
                    <span className="text-muted-foreground text-xs">—</span>
                  )}
                </td>
                <td className="px-4 py-3 hidden sm:table-cell">
                  {item.external_url ? (
                    <a
                      href={item.external_url}
                      target="_blank"
                      rel="noopener noreferrer"
                      aria-label={t("settings.orphans.openMetadata")}
                      className="inline-flex items-center gap-1 text-primary hover:underline"
                    >
                      <Icon name="externalLink" size="sm" mono />
                    </a>
                  ) : (
                    <span className="text-muted-foreground text-xs">—</span>
                  )}
                </td>
                <td className="px-4 py-3 text-right text-muted-foreground tabular-nums hidden md:table-cell whitespace-nowrap">
                  {item.age_days > 0
                    ? t("settings.orphans.ageDays", { days: item.age_days })
                    : t("settings.orphans.ageToday")}
                </td>
                <td className="px-4 py-3 text-right">
                  <Button
                    variant="outline"
                    size="xs"
                    onClick={() => setPendingDelete(item)}
                    disabled={busy || item.has_active_download}
                    title={
                      item.has_active_download
                        ? t("settings.orphans.activeDownloadWarning")
                        : undefined
                    }
                  >
                    <Icon name="trash" size="sm" className="mr-1.5" />
                    {t("settings.orphans.delete")}
                  </Button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      <Modal
        isOpen={pendingDelete !== null}
        onClose={() => (busy ? undefined : setPendingDelete(null))}
        title={t("settings.orphans.deleteConfirmTitle")}
        maxWidth="md"
        disableClose={busy}
        footer={
          <div className="flex justify-end gap-2">
            <Button variant="outline" onClick={() => setPendingDelete(null)} disabled={busy}>
              {t("common.cancel")}
            </Button>
            <Button
              variant="destructive"
              onClick={() => pendingDelete && remove([pendingDelete.id])}
              disabled={busy}
            >
              {t("settings.orphans.delete")}
            </Button>
          </div>
        }
      >
        <div className="space-y-3 px-5 py-4 text-sm text-muted-foreground">
          <p>{t("settings.orphans.deleteConfirm", { name: pendingDelete?.name ?? "" })}</p>
          {pendingDelete?.has_wishlist_release && (
            <p className="flex items-start gap-2 rounded-lg bg-warning/10 p-3 text-warning">
              <Icon name="warning" size="sm" mono className="mt-0.5 shrink-0" />
              {t("settings.orphans.wishlistReleaseWarning")}
            </p>
          )}
        </div>
      </Modal>

      <Modal
        isOpen={bulkOpen}
        onClose={() => (busy ? undefined : setBulkOpen(false))}
        title={t("settings.orphans.deletePageConfirmTitle")}
        maxWidth="md"
        disableClose={busy}
        footer={
          <div className="flex justify-end gap-2">
            <Button variant="outline" onClick={() => setBulkOpen(false)} disabled={busy}>
              {t("common.cancel")}
            </Button>
            <Button
              variant="destructive"
              onClick={() => remove(deletable.map((s) => s.id))}
              disabled={busy || deletable.length === 0}
            >
              {t("settings.orphans.deletePage")}
            </Button>
          </div>
        }
      >
        <div className="space-y-3 px-5 py-4 text-sm text-muted-foreground">
          <p>{t("settings.orphans.deletePageConfirm", { count: deletable.length })}</p>
          {hasActiveDownloadWarning && (
            <p className="flex items-start gap-2 rounded-lg bg-warning/10 p-3 text-warning">
              <Icon name="warning" size="sm" mono className="mt-0.5 shrink-0" />
              {t("settings.orphans.deletePageActiveWarning", { count: activeDownloadCount })}
            </p>
          )}
          {hasWishlistWarning && (
            <p className="flex items-start gap-2 rounded-lg bg-warning/10 p-3 text-warning">
              <Icon name="warning" size="sm" mono className="mt-0.5 shrink-0" />
              {t("settings.orphans.wishlistReleaseWarning")}
            </p>
          )}
        </div>
      </Modal>
    </>
  );
}
