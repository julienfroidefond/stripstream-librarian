import Link from "next/link";

import { buttonClasses } from "@/app/components/ui";
import { getServerTranslations } from "@/lib/i18n/server";

/**
 * Shared 404 content, rendered by both `app/not-found.tsx` (unmatched URLs, no
 * app chrome) and `app/(app)/not-found.tsx` (in-app `notFound()` calls, which
 * keep the header/nav).
 */
export async function NotFoundView() {
  const { t } = await getServerTranslations();

  return (
    <div className="flex min-h-[60vh] flex-col items-center justify-center px-4 text-center">
      <p aria-hidden="true" className="text-6xl font-bold tracking-tight text-muted-foreground/30">
        404
      </p>
      <h1 className="mt-4 text-2xl font-bold text-foreground">{t("notFound.title")}</h1>
      <p className="mt-2 max-w-md text-sm text-muted-foreground">{t("notFound.description")}</p>
      <Link href="/" className={buttonClasses({ className: "mt-6" })}>
        {t("notFound.backHome")}
      </Link>
    </div>
  );
}
