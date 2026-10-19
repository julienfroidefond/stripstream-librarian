"use client";

import { getBookFileDownloadUrl } from "@/lib/api";
import { fetchSharedFile, isIosStandalone } from "@/lib/download";
import { useTranslation } from "@/lib/i18n/context";
import { ActionsMenuItem, Icon, toast } from "./ui";

interface DownloadBookFileButtonProps {
  bookId: string;
  /**
   * Render a custom control instead of the default `ActionsMenuItem` — used by
   * the always-visible download shortcut of the card clusters.
   */
  children?: (control: {
    href: string;
    download: true;
    onClick: (e: React.MouseEvent<HTMLElement>) => Promise<void>;
  }) => React.ReactNode;
}

export function DownloadBookFileButton({ bookId, children }: DownloadBookFileButtonProps) {
  const { t } = useTranslation();
  const url = getBookFileDownloadUrl(bookId);

  async function handleClick(e: React.MouseEvent<HTMLElement>) {
    // Everywhere but an installed iOS web app, the plain link does the job.
    if (!isIosStandalone()) return;
    e.preventDefault();

    try {
      toast(t("bookDetail.downloadPreparing"), "info");
      const file = await fetchSharedFile(url);
      if (navigator.canShare?.({ files: [file] })) {
        await navigator.share({ files: [file] });
        return;
      }
      window.open(url, "_blank", "noopener");
    } catch (error) {
      // The user dismissed the share sheet: nothing else to do.
      if ((error as DOMException | undefined)?.name === "AbortError") return;
      window.open(url, "_blank", "noopener");
    }
  }

  if (children) {
    return <>{children({ href: url, download: true, onClick: handleClick })}</>;
  }

  return (
    <ActionsMenuItem
      href={url}
      download
      onClick={handleClick}
      icon={<Icon name="download" size="sm" className="text-info" />}
    >
      {t("bookDetail.downloadOriginal")}
    </ActionsMenuItem>
  );
}
