"use client";

import { useRouter } from "next/navigation";
import { useState, useTransition } from "react";
import { Button, Icon, toast } from "@/app/components/ui";
import { useTranslation } from "@/lib/i18n/context";

/**
 * Re-runs the diagnostics server component (fresh `GET /diagnostics`).
 *
 * Used both in the header and in the "API unreachable" panel, so the operator
 * can retry without reloading the whole page.
 */
export function RefreshDiagnosticsButton({ label }: { label?: string }) {
  const { t } = useTranslation();
  const router = useRouter();
  const [isPending, startTransition] = useTransition();

  return (
    <Button
      variant="outline"
      disabled={isPending}
      onClick={() => startTransition(() => router.refresh())}
    >
      <Icon name="refresh" size="sm" className={`mr-2 ${isPending ? "animate-spin" : ""}`} />
      {label ?? t("diagnostics.refresh")}
    </Button>
  );
}

/**
 * Purges the image page cache (memory + disk) through the existing settings
 * endpoint, then refreshes the displayed stats.
 */
export function PurgeCacheButton() {
  const { t } = useTranslation();
  const router = useRouter();
  const [isPurging, setIsPurging] = useState(false);

  async function handlePurge() {
    setIsPurging(true);
    try {
      const response = await fetch("/api/settings/cache/clear", { method: "POST" });
      const data = (await response.json()) as { success?: boolean; message?: string; error?: string };

      if (!response.ok || data.error || data.success === false) {
        toast(data.error ?? data.message ?? t("diagnostics.purgeError"), "error");
        return;
      }

      toast(data.message ?? t("diagnostics.purgeSuccess"), "success");
      router.refresh();
    } catch {
      toast(t("diagnostics.purgeError"), "error");
    } finally {
      setIsPurging(false);
    }
  }

  return (
    <Button variant="warning" size="sm" disabled={isPurging} onClick={handlePurge}>
      {isPurging ? (
        <>
          <Icon name="spinner" size="sm" className="mr-2 animate-spin" />
          {t("diagnostics.purging")}
        </>
      ) : (
        <>
          <Icon name="trash" size="sm" className="mr-2" />
          {t("diagnostics.purgeCache")}
        </>
      )}
    </Button>
  );
}

/**
 * Copies the whole diagnostics payload as JSON, to paste into a bug report.
 *
 * The payload is already sanitised server-side: it holds versions, sizes and
 * counters, never a token, DSN or credential.
 */
export function CopyDiagnosticsButton() {
  const { t } = useTranslation();
  const [isCopying, setIsCopying] = useState(false);

  async function handleCopy() {
    setIsCopying(true);
    try {
      const response = await fetch("/api/diagnostics");
      if (!response.ok) {
        throw new Error(`diagnostics failed (${response.status})`);
      }

      const data: unknown = await response.json();
      await navigator.clipboard.writeText(JSON.stringify(data, null, 2));
      toast(t("diagnostics.copySuccess"), "success");
    } catch {
      toast(t("diagnostics.copyError"), "error");
    } finally {
      setIsCopying(false);
    }
  }

  return (
    <Button variant="outline" disabled={isCopying} onClick={handleCopy}>
      <Icon name="document" size="sm" className="mr-2" />
      {t("diagnostics.copy")}
    </Button>
  );
}
