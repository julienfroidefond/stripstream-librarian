"use client";

import { useState } from "react";
import { useRouter } from "next/navigation";
import { Button, Icon, Modal } from "./ui";
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

      <Modal isOpen={showConfirm} onClose={() => setShowConfirm(false)} maxWidth="sm">
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
      </Modal>
    </>
  );
}
