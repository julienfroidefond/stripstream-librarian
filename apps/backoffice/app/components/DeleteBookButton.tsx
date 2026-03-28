"use client";

import { useState } from "react";
import { createPortal } from "react-dom";
import { useRouter } from "next/navigation";
import { Button, Icon } from "./ui";
import { useTranslation } from "@/lib/i18n/context";

export function DeleteBookButton({ bookId, libraryId }: { bookId: string; libraryId: string }) {
  const { t } = useTranslation();
  const router = useRouter();
  const [showConfirm, setShowConfirm] = useState(false);
  const [deleting, setDeleting] = useState(false);

  async function handleDelete() {
    setDeleting(true);
    setShowConfirm(false);
    try {
      const resp = await fetch(`/api/books/${bookId}`, { method: "DELETE" });
      if (resp.ok) {
        router.push(`/libraries/${libraryId}/series`);
      }
    } finally {
      setDeleting(false);
    }
  }

  return (
    <>
      <Button
        variant="destructive"
        size="sm"
        onClick={() => setShowConfirm(true)}
        disabled={deleting}
      >
        {deleting ? <Icon name="spinner" size="sm" className="animate-spin" /> : <Icon name="trash" size="sm" />}
        <span className="ml-1.5">{t("bookDetail.delete")}</span>
      </Button>

      {showConfirm && createPortal(
        <>
          <div className="fixed inset-0 bg-black/30 backdrop-blur-sm z-50" onClick={() => setShowConfirm(false)} />
          <div className="fixed inset-0 flex items-center justify-center z-50 p-4">
            <div className="bg-card border border-border/50 rounded-xl shadow-2xl w-full max-w-sm overflow-hidden animate-in fade-in zoom-in-95 duration-200">
              <div className="p-6">
                <h3 className="text-lg font-semibold text-foreground mb-2">
                  {t("bookDetail.delete")}
                </h3>
                <p className="text-sm text-muted-foreground">
                  {t("bookDetail.confirmDelete")}
                </p>
              </div>
              <div className="flex justify-end gap-2 px-6 pb-6">
                <Button variant="outline" size="sm" onClick={() => setShowConfirm(false)}>
                  {t("common.cancel")}
                </Button>
                <Button variant="destructive" size="sm" onClick={handleDelete}>
                  {t("bookDetail.delete")}
                </Button>
              </div>
            </div>
          </div>
        </>,
        document.body
      )}
    </>
  );
}
