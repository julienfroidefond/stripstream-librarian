"use client";

import { useState } from "react";
import { useRouter } from "next/navigation";
import { Button, Icon, Modal } from "./ui";
import { useTranslation } from "@/lib/i18n/context";
import type { TranslationKey } from "@/lib/i18n/fr";

interface DeleteConfirmButtonProps {
  endpoint: string;
  titleKey: TranslationKey;
  confirmKey: TranslationKey;
  labelKey: TranslationKey;
  children?: (open: () => void) => React.ReactNode;
}

export function DeleteConfirmButton({ endpoint, titleKey, confirmKey, labelKey, children }: DeleteConfirmButtonProps) {
  const { t } = useTranslation();
  const router = useRouter();
  const [showConfirm, setShowConfirm] = useState(false);
  const [deleting, setDeleting] = useState(false);

  async function handleDelete() {
    setDeleting(true);
    setShowConfirm(false);
    try {
      const resp = await fetch(endpoint, { method: "DELETE" });
      if (resp.ok) {
        router.refresh();
        router.push("/series");
      }
    } finally {
      setDeleting(false);
    }
  }

  const open = () => setShowConfirm(true);

  return (
    <>
      {children ? (
        children(open)
      ) : (
        <button
          type="button"
          onClick={open}
          disabled={deleting}
          className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg border border-destructive/30 bg-destructive/10 text-destructive text-sm font-medium hover:bg-destructive/20 transition-colors disabled:opacity-50"
        >
          {deleting ? <Icon name="spinner" size="sm" className="animate-spin" /> : <Icon name="trash" size="sm" />}
          {t(labelKey)}
        </button>
      )}

      <Modal isOpen={showConfirm} onClose={() => setShowConfirm(false)} maxWidth="sm">
        <div className="p-6">
          <h3 className="text-lg font-semibold text-foreground mb-2">
            {t(titleKey)}
          </h3>
          <p className="text-sm text-muted-foreground">
            {t(confirmKey)}
          </p>
        </div>
        <div className="flex justify-end gap-2 px-6 pb-6">
          <Button variant="outline" size="sm" onClick={() => setShowConfirm(false)}>
            {t("common.cancel")}
          </Button>
          <Button variant="destructive" size="sm" onClick={handleDelete}>
            {t(labelKey)}
          </Button>
        </div>
      </Modal>
    </>
  );
}
