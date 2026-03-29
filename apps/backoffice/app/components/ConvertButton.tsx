"use client";

import { useState } from "react";
import Link from "next/link";
import { Button } from "./ui";
import { useTranslation } from "../../lib/i18n/context";

interface ConvertButtonProps {
  bookId: string;
}

type ConvertState =
  | { type: "idle" }
  | { type: "loading" }
  | { type: "success"; jobId: string }
  | { type: "error"; message: string };

export function ConvertButton({ bookId }: ConvertButtonProps) {
  const { t } = useTranslation();
  const [state, setState] = useState<ConvertState>({ type: "idle" });

  const handleConvert = async () => {
    setState({ type: "loading" });
    try {
      const res = await fetch(`/api/books/${bookId}/convert`, { method: "POST" });
      if (!res.ok) {
        const body = await res.json().catch(() => ({ error: res.statusText }));
        setState({ type: "error", message: body.error || t("convert.failed") });
        return;
      }
      const job = await res.json();
      setState({ type: "success", jobId: job.id });
    } catch (err) {
      setState({ type: "error", message: err instanceof Error ? err.message : t("convert.unknownError") });
    }
  };

  if (state.type === "success") {
    return (
      <div className="flex items-center gap-2 text-sm text-success">
        <span>{t("convert.started")}</span>
        <Link href={`/jobs/${state.jobId}`} className="text-primary hover:underline font-medium">
          {t("convert.viewJob")}
        </Link>
      </div>
    );
  }

  if (state.type === "error") {
    return (
      <div className="flex flex-col gap-1">
        <span className="text-sm text-destructive">{state.message}</span>
        <button
          className="text-xs text-muted-foreground hover:underline text-left"
          onClick={() => setState({ type: "idle" })}
        >
          {t("common.close")}
        </button>
      </div>
    );
  }

  return (
    <button
      type="button"
      onClick={handleConvert}
      disabled={state.type === "loading"}
      className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg border border-border bg-card text-sm font-medium text-muted-foreground hover:text-foreground hover:border-primary transition-colors disabled:opacity-50"
    >
      {state.type === "loading" ? t("convert.converting") : t("convert.convertToCbz")}
    </button>
  );
}
