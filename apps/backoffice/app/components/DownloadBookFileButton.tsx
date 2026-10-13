"use client";

import { getBookFileDownloadUrl } from "@/lib/api";
import { useTranslation } from "@/lib/i18n/context";
import { ActionsMenuItem, Icon } from "./ui";

interface DownloadBookFileButtonProps {
  bookId: string;
}

export function DownloadBookFileButton({ bookId }: DownloadBookFileButtonProps) {
  const { t } = useTranslation();

  return (
    <ActionsMenuItem
      href={getBookFileDownloadUrl(bookId)}
      download
      icon={<Icon name="download" size="sm" className="text-sky-500" />}
    >
      {t("bookDetail.downloadOriginal")}
    </ActionsMenuItem>
  );
}
