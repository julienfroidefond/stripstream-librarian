import { Card, CardHeader, CardTitle, CardDescription, CardContent } from "@/app/components/ui";
import type { TranslateFunction } from "@/lib/i18n/dictionaries";

interface JobError {
  id: string;
  file_path: string;
  error_message: string;
  created_at: string;
}

export function JobErrorsCard({ errors, t, locale }: { errors: JobError[]; t: TranslateFunction; locale: string }) {
  if (errors.length === 0) return null;

  return (
    <Card className="lg:col-span-2">
      <CardHeader>
        <CardTitle>{t("jobDetail.fileErrors", { count: String(errors.length) })}</CardTitle>
        <CardDescription>{t("jobDetail.fileErrorsDesc")}</CardDescription>
      </CardHeader>
      <CardContent className="space-y-2 max-h-80 overflow-y-auto">
        {errors.map((error) => (
          <div key={error.id} className="p-3 bg-destructive/10 rounded-lg border border-destructive/20">
            <code className="block text-sm font-mono text-destructive mb-1">{error.file_path}</code>
            <p className="text-sm text-destructive/80">{error.error_message}</p>
            <span className="text-xs text-muted-foreground">{new Date(error.created_at).toLocaleString(locale)}</span>
          </div>
        ))}
      </CardContent>
    </Card>
  );
}
