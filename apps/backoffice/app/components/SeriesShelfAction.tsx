"use client";

import { useState } from "react";
import { useRouter } from "next/navigation";

import { useTranslation } from "@/lib/i18n/context";
import { ActionsMenuItem, Icon, toast } from "./ui";

interface SeriesShelfActionProps {
  seriesId: string;
  initialIsShelved: boolean;
}

/**
 * Reader-scoped "put aside" toggle exposed in the series action menus.
 * Only rendered when a reader is impersonated in the backoffice.
 */
export function SeriesShelfAction({ seriesId, initialIsShelved }: SeriesShelfActionProps) {
  const { t } = useTranslation();
  const router = useRouter();
  const [isShelved, setIsShelved] = useState(initialIsShelved);
  const [pending, setPending] = useState(false);

  async function handleClick() {
    if (pending) return;
    setPending(true);

    try {
      const response = await fetch(`/api/series/${seriesId}/shelf`, {
        method: isShelved ? "DELETE" : "PUT",
      });

      if (!response.ok) {
        const body = await response.json().catch(() => null);
        throw new Error(body?.error || t("seriesShelf.error"));
      }

      const nextIsShelved = !isShelved;
      setIsShelved(nextIsShelved);
      toast(t(nextIsShelved ? "seriesShelf.shelved" : "seriesShelf.resumed"), "success");
      router.refresh();
    } catch (error) {
      toast(error instanceof Error && error.message ? error.message : t("seriesShelf.error"), "error");
    } finally {
      setPending(false);
    }
  }

  return (
    <ActionsMenuItem
      icon={
        isShelved
          ? <Icon name="play" size="sm" />
          : <Icon name="bookmark" size="sm" />
      }
      onClick={handleClick}
      disabled={pending}
    >
      {pending
        ? t("common.loading")
        : isShelved
          ? t("seriesShelf.resume")
          : t("seriesShelf.shelve")}
    </ActionsMenuItem>
  );
}
