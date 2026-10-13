"use client";

import { getBookFileDownloadUrl } from "@/lib/api";
import { fetchSharedFile, isIosStandalone } from "@/lib/download";
import { useTranslation } from "@/lib/i18n/context";
import { ActionsMenuItem, Icon, toast } from "./ui";

interface DownloadBookFileButtonProps {
  bookId: string;
}

export function DownloadBookFileButton({ bookId }: DownloadBookFileButtonProps) {
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

  return (
    <ActionsMenuItem
      href={url}
      download
      onClick={handleClick}
      icon={<Icon name="download" size="sm" className="text-sky-500" />}
    >
      {t("bookDetail.downloadOriginal")}
    </ActionsMenuItem>
  );
}
