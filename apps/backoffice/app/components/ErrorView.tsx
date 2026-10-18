"use client";

import { useEffect } from "react";
import Link from "next/link";

import { Button, buttonClasses } from "@/app/components/ui";
import { useTranslation } from "@/lib/i18n/context";

interface ErrorViewProps {
  error: Error & { digest?: string };
  reset: () => void;
}

/**
 * Shared error boundary content, rendered by `app/error.tsx` and
 * `app/(app)/error.tsx`. Stays a Client Component: `reset` only exists
 * client-side.
 */
export function ErrorView({ error, reset }: ErrorViewProps) {
  const { t } = useTranslation();

  useEffect(() => {
    console.error(error);
  }, [error]);

  return (
    <div className="flex min-h-[60vh] flex-col items-center justify-center px-4 text-center animate-fade-in-up">
      <h1 className="text-2xl font-bold text-foreground">{t("error.title")}</h1>
      <p className="mt-2 max-w-md text-sm text-muted-foreground">{t("error.description")}</p>
      {error.digest && (
        <p className="mt-1 text-xs text-muted-foreground/70">
          {t("error.reference", { digest: error.digest })}
        </p>
      )}
      <div className="mt-6 flex flex-wrap items-center justify-center gap-3">
        <Button onClick={reset}>{t("error.retry")}</Button>
        <Link href="/" className={buttonClasses({ variant: "outline" })}>
          {t("error.backHome")}
        </Link>
      </div>
    </div>
  );
}
